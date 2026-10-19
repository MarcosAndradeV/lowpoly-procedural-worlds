# lppw-render

Low-poly 3D graphics presentation engine for procedural worlds.

## Features

- **Chunked Terrain Rendering**: Frustum-culled, 7x7 toroidal GpuMesh ring buffer with dirty-state caching and seamless boundary normals.
- **Dynamic Water / Lava**: Faceted wave deformation, ambient & specular sun highlights, and retained GPU VBO mesh rendering.
- **Atmospheric Sky**: Dynamic horizon-to-zenith color gradient with time-of-day progression (Noon, Sunset, Night) and low-poly drifting clouds.
- **Vegetation Props**: Biome-tailored low-poly trees (pine, oak, palm), cacti, mushrooms, crystal spires, boulders, and reeds.
- **Architectural Rendering**: Complete 3D structures for fantasy towns (houses, market stalls, windmills, bridges) and landmarks (Stonehenge, Lighthouse, Wizard Tower, World Tree, Ancient Temple).
- **Explorer Camera**: Dual-mode (Free Fly 6-DOF and Ground Walk terrain following) with 5-plane view frustum culling.
