# Vulkan Vibe Coding

A cross-platform Rust application demonstrating Vulkan rendering with a bouncing circle animation. Vibe coded with Grok 3 and Claude 3.7 Sonnet to teach myself Rust and rendering pipelines. 

## Overview

This Rust program:

- Uses the `winit` library to create a cross-platform window
- Uses the `ash` crate to interface with Vulkan for rendering
- Sets platform-specific window icons (.ico for Windows, .icns for macOS)
- Renders a moving circle that bounces off window edges
- Handles window resizing and proper Vulkan resource management

## Dependencies

```toml
[dependencies]                # Libraries your project needs to run
winit = "0.30.9"              # For creating and managing windows
ico = "0.4"                   # For handling .ico (icon) files
ash = "0.38"                  # For interacting with Vulkan (the graphics API)
icns = "0.3"                  # For macOS ICNS parsing at runtime
glam = "0.27"                 # For vector math and linear algebra
bytemuck = "1.22.0"           # For casting between Rust types and byte slices

[build-dependencies]          # Libraries needed only during the build process
winresource = "0.1.19"        # For embedding Windows-specific resources (like icons) into the binary
```

## Project Structure

- `build.rs` - Platform-specific build configuration
    - Embeds Windows icon into executable
    - Checks for macOS icon existence
    - Sets up rebuild triggers for asset changes

- `main.rs` - Core application logic including:
    - Window creation and management
    - Vulkan initialization and rendering
    - Circle physics and animation
    - Event handling and cleanup

- `assets/`
    - `icon.ico` - Windows application icon
    - `icon.icns` - macOS application icon
    - `vert.spv` - Precompiled vertex shader
    - `frag.spv` - Precompiled fragment shader

## Key Features

### Window Management
- 800x600 window titled "winit/Vulkan Window - Moving Circle"
- Platform-specific icon handling
- Event handling for close, resize, and redraw events

### Vulkan Implementation
- Complete Vulkan rendering pipeline setup
- Vertex buffer creation for circle geometry
- Swapchain management for smooth rendering
- Proper resource cleanup

### Animation
- Circle bounces off window edges
- ~60 FPS rendering with fixed timestep
- Simple physics with position and velocity vectors

## Technical Details

### Circle Rendering
The circle is approximated as a triangle fan with 32 segments, with vertex positions calculated using trigonometry.

### Vulkan Pipeline
1. Creates Vulkan instance with required extensions
2. Selects suitable physical device and queue
3. Creates logical device with swapchain extension
4. Sets up swapchain, image views, and render pass
5. Initializes command buffers and synchronization objects
6. Creates vertex buffer and graphics pipeline
7. Renders with orthographic projection

### Cross-Platform Compatibility
- Windows-specific surface creation and icon embedding
- macOS-specific icon loading at runtime
- Consistent rendering across platforms

## Purpose

This project serves as an educational example demonstrating:
- Low-level graphics programming with Vulkan
- Cross-platform window management
- Basic animation and rendering techniques
- Proper resource management in graphics applications