# Installation

This guide will walk you through adding `bevy_point_cloud` to your Bevy project.

## Prerequisites

Before starting, ensure you have a working Bevy setup. If you are new to Bevy or need to configure your system's graphics dependencies, please follow the official [Bevy Setup and Installation Guide](https://bevyengine.org/learn/quick-start/getting-started/setup/).

## Compatibility Matrix

Because Bevy evolves rapidly, please ensure your project's Bevy version matches the plugin's target version:

| Bevy Version | `bevy_point_cloud` Version / Branch |
| :--- | :--- |
| **0.19** | `branch = "reboot"` (Development) |
| *To Be Announced* | `0.1.0` (Future crates.io release) |

## Adding the Dependency

Since the plugin is actively developed and not yet published on [crates.io](https://crates.io), you must pull it directly from the Git repository.

### Option 1: Using Cargo CLI (Recommended)

Run the following command in your project root to automatically add the Git dependency to your `Cargo.toml`:

```bash
cargo add bevy_point_cloud --git https://github.com/rlamarche/bevy_pointcloud --branch reboot
```

### Option 2: Manual `Cargo.toml` Editing

Open your `Cargo.toml` file and add `bevy_point_cloud` under the `[dependencies]` section:

```toml
[dependencies]
bevy_point_cloud = { git = "https://github.com/rlamarche/bevy_pointcloud", branch = "reboot" }
```

> 💡 **Note on Crates.io:** Once the plugin reaches a stable milestone, standard versioning will be available via `bevy_point_cloud = "0.1"`.

## Performance Tip

Point clouds involve handling millions of vertices and heavy GPU data streaming. To avoid stuttering and low framerates during development, always run your project with optimizations enabled:

```bash
# Run with compiler optimizations
cargo run --release
```

If you want to keep fast compilation times for your own code but need full performance for dependencies (like Bevy and this plugin), add the following profile optimization to your `Cargo.toml`:

```toml
# Optimize dependencies even in dev mode
[profile.dev.package."*"]
opt-level = 3
```
