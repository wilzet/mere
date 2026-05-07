use std::time::Instant;

pub(crate) type ResolvedSpan = (String, u128, u128);

pub struct CpuSpan {
    pub(crate) name: &'static str,
    pub(crate) start: Instant,
    pub(crate) end: Instant,
}

struct GpuSpan {
    name: &'static str,
    start_query: u32,
    end_query: u32,
}

pub trait ResolveTimestamp<'a> {
    fn into(query_set: &'a wgpu::QuerySet, id: u32) -> Self;
}

impl<'a> ResolveTimestamp<'a> for wgpu::ComputePassTimestampWrites<'a> {
    fn into(query_set: &'a wgpu::QuerySet, id: u32) -> Self {
        Self {
            query_set,
            beginning_of_pass_write_index: Some(id),
            end_of_pass_write_index: Some(id + 1),
        }
    }
}

impl<'a> ResolveTimestamp<'a> for wgpu::RenderPassTimestampWrites<'a> {
    fn into(query_set: &'a wgpu::QuerySet, id: u32) -> Self {
        Self {
            query_set,
            beginning_of_pass_write_index: Some(id),
            end_of_pass_write_index: Some(id + 1),
        }
    }
}

struct FrameData {
    query_count: u32,
    spans: Vec<GpuSpan>,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    // This is the key: we track if this specific frame has been submitted yet
    submission_index: Option<wgpu::SubmissionIndex>,
}

pub struct Profiler {
    enabled: bool,
    cpu_spans: Vec<CpuSpan>,
    current_cpu_start: Option<(Instant, &'static str)>,
    timestamp_queries: wgpu::QuerySet,
    frames: [FrameData; 2],
    frame_index: usize,
}

const MAX_QUERIES: u32 = 64;

impl Profiler {
    pub fn new(device: &wgpu::Device) -> Self {
        let create_frame = |i| FrameData {
            query_count: 0,
            spans: Vec::new(),
            resolve_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("profiler_resolve_buffer_{}", i)),
                size: (MAX_QUERIES as u64) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("profiler_readback_buffer_{}", i)),
                size: (MAX_QUERIES as u64) * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            submission_index: None,
        };

        Self {
            enabled: true,
            cpu_spans: Vec::new(),
            current_cpu_start: None,
            timestamp_queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("profiler_queries"),
                ty: wgpu::QueryType::Timestamp,
                count: MAX_QUERIES,
            }),
            frames: [create_frame(0), create_frame(1)],
            frame_index: 0,
        }
    }

    pub fn begin<'a, T: ResolveTimestamp<'a>>(&'a mut self, name: &'static str) -> Option<T> {
        if !self.enabled {
            return None;
        }

        self.current_cpu_start = Some((Instant::now(), name));

        let frame = &mut self.frames[self.frame_index];
        if frame.query_count + 2 > MAX_QUERIES {
            return None;
        }

        let id = frame.query_count;
        frame.query_count += 2;

        frame.spans.push(GpuSpan {
            name,
            start_query: id,
            end_query: id + 1,
        });

        Some(ResolveTimestamp::into(&self.timestamp_queries, id))
    }

    pub fn end(&mut self) {
        if let Some((start, name)) = self.current_cpu_start.take() {
            self.cpu_spans.push(CpuSpan {
                name,
                start,
                end: Instant::now(),
            });
        }
    }

    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if !self.enabled {
            return;
        }

        let frame = &self.frames[self.frame_index];
        if frame.query_count > 0 {
            encoder.resolve_query_set(
                &self.timestamp_queries,
                0..frame.query_count,
                &frame.resolve_buffer,
                0,
            );
            encoder.copy_buffer_to_buffer(
                &frame.resolve_buffer,
                0,
                &frame.readback_buffer,
                0,
                (frame.query_count as u64) * 8,
            );
        }
    }

    pub fn set_submission_index(&mut self, index: wgpu::SubmissionIndex) {
        if !self.enabled {
            return;
        }

        self.frames[self.frame_index].submission_index = Some(index);
    }

    pub fn cpu_spans(&self) -> Vec<ResolvedSpan> {
        if !self.enabled {
            return vec![];
        }

        let spans = &self.cpu_spans;
        if spans.is_empty() {
            return vec![];
        }

        let frame_start = spans[0].start;

        spans
            .iter()
            .map(|s| {
                let start = (s.start - frame_start).as_nanos();
                let end = (s.end - frame_start).as_nanos();

                (s.name.to_string(), start, end)
            })
            .collect()
    }

    pub fn gpu_spans(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<ResolvedSpan> {
        if !self.enabled {
            return vec![];
        }

        let prev_idx = (self.frame_index + 1) % 2;
        let frame = &self.frames[prev_idx];

        let submission_index = match &frame.submission_index {
            Some(idx) if frame.query_count > 0 => idx,
            _ => return vec![],
        };

        match device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission_index.clone()),
            timeout: None,
        }) {
            Err(err) => {
                mere_log::error!("Polling error: {err}");
                return vec![];
            }
            _ => (),
        }

        let buffer_slice = frame
            .readback_buffer
            .slice(..(frame.query_count as u64) * 8);
        let (tx, rx) = std::sync::mpsc::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });

        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        if let Ok(Ok(_)) = rx.try_recv() {
            let data = buffer_slice.get_mapped_range();
            let timestamps = bytemuck::cast_slice(&data);

            let period = queue.get_timestamp_period() as f64;
            let base_tick = timestamps.get(0).copied().unwrap_or(0u64);

            let out = frame
                .spans
                .iter()
                .map(|span| {
                    let start_tick = timestamps[span.start_query as usize];
                    let end_tick = timestamps[span.end_query as usize];

                    let start_ns = (start_tick.saturating_sub(base_tick)) as f64 * period;
                    let end_ns = (end_tick.saturating_sub(base_tick)) as f64 * period;

                    (span.name.to_string(), start_ns as u128, end_ns as u128)
                })
                .collect();

            drop(data);
            frame.readback_buffer.unmap();

            out
        } else {
            vec![]
        }
    }

    pub fn finish_frame(&mut self) {
        self.cpu_spans.clear();

        self.frame_index = (self.frame_index + 1) % 2;

        let next_frame = &mut self.frames[self.frame_index];
        next_frame.query_count = 0;
        next_frame.spans.clear();
        next_frame.submission_index = None;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.cpu_spans.clear();
            for frame in &mut self.frames {
                frame.query_count = 0;
                frame.spans.clear();
                frame.submission_index = None;
            }
        }
    }
}
