# WebGL, WebGPU & Web Workers Roadmap

The plugin is designed with broad platform compatibility in mind, specifically targeting the web ecosystem:
* **WebGL 2.0 Support:** To ensure the plugin runs everywhere today, the current architecture avoids advanced modern GPU features that would break WebGL 2.0 compatibility. This is the main reason visibility tracking is currently backed by a specialized texture rather than advanced buffer structures.
* **Future WebGPU Roadmap:** Once WebGPU reaches General Availability (GA) and wider adoption, this texture-based approach could be upgraded to use `StorageBuffer` (SSBO). This will completely bypass texture size limitations and significantly improve tracking efficiency on modern hardware while maintaining a fallback for WebGL 2.0.
* **Multi-threaded Web Parsing:** Future roadmap entries include shifting chunk decompression and data parsing out of the main UI thread. By leveraging Web Workers alongside modern WASM multithreading capabilities, processing will happen on a background thread pool to ensure a locked 60+ FPS experience during heavy streaming.
