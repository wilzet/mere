# MeRe: Meshlet Rendering Engine

**MeRe** is a rendering engine built with **Rust** and **wgpu**.

[//]: # (It leverages modern GPU features to implement virtual geometry, enabling efficient culling and level-of-detail management for highly detailed scenes.)

## Description
Traditional vertex processing can become a bottleneck when dealing with high-poly geometry. **MeRe** addresses this by partitioning meshes into *meshlets* (small clusters of geometry), allowing for early rejection of geometry at the cluster level via frustum and occlusion culling. By utilizing compute shaders, all of the culling is allowed to be processed on the GPU.

## Installation & Dependencies

### Prerequisites
1.  **Rust Toolchain:** Install the latest stable version via [rustup.rs](https://rustup.rs/).
2.  **GPU Drivers:** Ensure your drivers support **Vulkan 1.2+**, **Metal**, or **DirectX 12**.

### Build and run the Project
Clone the repository:
```bash
git clone https://git.chalmers.se/wilmerz1/tda205-project.git
cd tda205-project
```

Then build and run the project with the default scene:
```bash
cargo run # or `cargo run --release`
```

## References
* [Nanite presentation by Brian Karis](https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf)
* [JGLRXAVPOK's blog](https://jglrxavpok.github.io/2023/11/12/recreating-nanite-the-plan.html)
* [Virtual Geometry in Bevy 0.14 by jms55](https://jms55.github.io/posts/2024-06-09-virtual-geometry-bevy-0-14/)
* [Mesh Shading by chaoticbob](https://chaoticbob.github.io/2024/01/24/mesh-shading-part-1.html)
