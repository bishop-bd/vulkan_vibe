use ash::vk;
use bytemuck;
use glam::{Mat4, Vec2};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
#[cfg(target_os = "macos")]
use objc::{
    rc::autoreleasepool,
    runtime::{Object, YES, NO},
    class,
    msg_send,
    sel,
    sel_impl,
};

#[repr(C)]
struct Vertex {
    position: [f32; 2],
}

fn create_circle_vertices(radius: f32, segments: u32) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(segments as usize + 2);
    vertices.push(Vertex {
        position: [0.0, 0.0],
    }); // Center
    for i in 0..=segments {
        let angle = i as f32 * 2.0 * std::f32::consts::PI / segments as f32;
        vertices.push(Vertex {
            position: [radius * angle.cos(), radius * angle.sin()],
        });
    }
    vertices
}

struct App {
    window: Option<Window>,
    entry: ash::Entry,
    instance: Option<ash::Instance>,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: Option<ash::Device>,
    queue: vk::Queue,
    swapchain: vk::SwapchainKHR,
    swapchain_ext: Option<ash::khr::swapchain::Device>,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,
    extent: vk::Extent2D,
    circle_position: Vec2,
    circle_velocity: Vec2,
    last_title_update: std::time::Instant,
    frame_count: u32,
    fps: f32,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("winit/Vulkan Window - Moving Circle")
                    .with_inner_size(LogicalSize::new(800, 600)),
            )
            .expect("Failed to create window");

        println!("Window created successfully");

        #[cfg(target_os = "windows")]
        {
            use std::io::Cursor;
            use winit::window::Icon;
            use ico::IconDir;
            const ICON_DATA: &[u8] = include_bytes!("../assets/icon.ico");

            let mut cursor = Cursor::new(ICON_DATA);
            let ico = IconDir::read(&mut cursor).expect("Failed to read icon data");
            let entry = ico
                .entries()
                .iter()
                .find(|e| e.width() == 64 && e.height() == 64)
                .expect("No 16x16 icon found in assets/icon.ico");
            let icon_image = entry.decode().expect("Failed to decode icon image");
            let rgba = icon_image.rgba_data().to_vec();
            let width = icon_image.width();
            let height = icon_image.height();
            let icon =
                Icon::from_rgba(rgba, width, height).expect("Failed to create icon from RGBA data");
            window.set_window_icon(Some(icon));
            println!("Set Windows window icon");
        }
        #[cfg(target_os = "macos")]
        {
            use std::io::Cursor;
            use icns::IconFamily;
            use winit::window::Icon;
            const ICNS_DATA: &[u8] = include_bytes!("../assets/icon.icns");

            let mut cursor = Cursor::new(ICNS_DATA);
            let icon_family = IconFamily::read(&mut cursor).expect("Failed to read icon.icns");
            match icon_family.get_icon_with_type(icns::IconType::RGBA32_512x512) {
                Ok(image) => {
                    let rgba = image.data().to_vec();
                    let width = image.width();
                    let height = image.height();
                    let icon = Icon::from_rgba(rgba, width, height)
                        .expect("Failed to create icon from ICNS data");
                    window.set_window_icon(Some(icon));
                    println!("Set macOS window icon");
                }
                Err(e) => {
                    println!(
                        "cargo:warning=Failed to get 16x16 icon from assets/icon.icns: {:?}",
                        e
                    );
                }
            }
        }

        self.window = Some(window);
        self.init_vulkan();
        println!("Resumed event completed");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Close requested, exiting");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.update_circle_position();
                self.render();
            }
            WindowEvent::Resized(_new_size) => {
                self.recreate_swapchain();
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => {}
        }
    }
}

impl App {
    fn init_vulkan(&mut self) {
        println!("Initializing Vulkan");
        use std::ffi::{CStr, CString};

        let available_extensions = unsafe {
            self.entry
                .enumerate_instance_extension_properties(None)
                .expect("Failed to enumerate instance extensions")
        };
        println!("Available Vulkan extensions:");
        for ext in &available_extensions {
            let ext_name = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };
            println!("- {:?}", ext_name);
        }

        let app_info = vk::ApplicationInfo {
            api_version: vk::make_api_version(0, 1, 0, 0),
            ..Default::default()
        };

        let mut instance_extension_names = vec![
            CString::new("VK_KHR_surface").unwrap(),
            CString::new("VK_KHR_portability_enumeration").unwrap(),
        ];
        #[cfg(target_os = "windows")]
        instance_extension_names.push(CString::new("VK_KHR_win32_surface").unwrap());
        #[cfg(target_os = "macos")]
        instance_extension_names.push(CString::new("VK_EXT_metal_surface").unwrap());
        #[cfg(target_os = "linux")]
        {
            instance_extension_names.push(CString::new("VK_KHR_xlib_surface").unwrap());
            instance_extension_names.push(CString::new("VK_KHR_wayland_surface").unwrap());
        }

        let instance_extension_names_ptrs: Vec<*const std::os::raw::c_char> =
            instance_extension_names
                .iter()
                .map(|c| c.as_ptr())
                .collect();

        let instance_create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            enabled_extension_count: instance_extension_names_ptrs.len() as u32,
            pp_enabled_extension_names: instance_extension_names_ptrs.as_ptr(),
            flags: vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR,
            ..Default::default()
        };

        println!(
            "Attempting to create Vulkan instance with extensions: {:?}",
            instance_extension_names
        );
        match unsafe { self.entry.create_instance(&instance_create_info, None) } {
            Ok(instance) => {
                self.instance = Some(instance);
                println!("Vulkan instance created successfully");
            }
            Err(e) => {
                println!("Failed to create Vulkan instance: {:?}", e);
                return;
            }
        }

        // Surface creation
        println!("Creating Vulkan surface");
        let window = self.window.as_ref().unwrap();
        println!("Got window reference");
        let raw_window_handle = window.window_handle().expect("Failed to get window handle").as_raw();
        println!("Got raw window handle");
        match raw_window_handle {
            #[cfg(target_os = "windows")]
            RawWindowHandle::Win32(handle) => {
                let surface_create_info = vk::Win32SurfaceCreateInfoKHR {
                    hinstance: handle.hinstance.map(|nz| nz.get()).unwrap_or(0),
                    hwnd: handle.hwnd.get(),
                    ..Default::default()
                };
                let win32_surface_instance = ash::khr::win32_surface::Instance::new(&self.entry, self.instance.as_ref().unwrap());
                match unsafe { win32_surface_instance.create_win32_surface(&surface_create_info, None) } {
                    Ok(surface) => {
                        self.surface = surface;
                        println!("Vulkan surface created successfully (Windows)");
                    }
                    Err(e) => {
                        println!("Failed to create Vulkan surface: {:?}", e);
                        return;
                    }
                }
            }
            #[cfg(target_os = "macos")]
            RawWindowHandle::AppKit(handle) => {
                #[cfg(target_os = "macos")]
                use ash::ext::metal_surface;

                #[cfg(target_os = "macos")]
                #[allow(unexpected_cfgs)]
                autoreleasepool(|| {
                    let ns_view = handle.ns_view.as_ptr() as *mut Object;
                    println!("NSView pointer: {:p}", ns_view);

                    // Create a CAMetalLayer
                    let metal_layer: *mut Object = unsafe { msg_send![class!(CAMetalLayer), layer] };
                    println!("Created CAMetalLayer: {:p}", metal_layer);

                    // Set the layer on the NSView
                    unsafe {
                        let () = msg_send![ns_view, setLayer: metal_layer];
                        let () = msg_send![ns_view, setWantsLayer: YES];
                        let () = msg_send![metal_layer, setDisplaySyncEnabled: NO];
                    }
                    println!("Set CAMetalLayer on NSView");

                    // Create Vulkan surface with the CAMetalLayer
                    let surface_create_info = vk::MetalSurfaceCreateInfoEXT {
                        s_type: vk::StructureType::METAL_SURFACE_CREATE_INFO_EXT,
                        p_next: std::ptr::null(),
                        flags: vk::MetalSurfaceCreateFlagsEXT::empty(),
                        p_layer: metal_layer as *const _,
                        _marker: std::marker::PhantomData,
                    };
                    println!("Building surface create info");
                    let metal_surface_instance = metal_surface::Instance::new(&self.entry, self.instance.as_ref().unwrap());
                    println!("Creating metal surface instance");
                    println!("Attempting to create metal surface");
                    match unsafe { metal_surface_instance.create_metal_surface(&surface_create_info, None) } {
                        Ok(surface) => {
                            self.surface = surface;
                            println!("Vulkan surface created successfully (macOS)");
                        }
                        Err(e) => {
                            println!("Failed to create Vulkan surface: {:?}", e);
                            return;
                        }
                    }
                });
            }
            #[cfg(target_os = "linux")]
            RawWindowHandle::Xlib(handle) => {
                let display_handle = self.window.as_ref().unwrap().display_handle().expect("Failed to get display handle");
                let xlib_display_handle = match display_handle.as_raw() {
                    RawDisplayHandle::Xlib(xlib) => xlib,
                    _ => panic!("Expected Xlib display handle for X11 window"),
                };
                let display = xlib_display_handle.display.unwrap().as_ptr();
                let surface_create_info = vk::XlibSurfaceCreateInfoKHR {
                    dpy: display,
                    window: handle.window,
                    ..Default::default()
                };
                let xlib_surface_instance = ash::khr::xlib_surface::Instance::new(&self.entry, self.instance.as_ref().unwrap());
                self.surface = unsafe { xlib_surface_instance.create_xlib_surface(&surface_create_info, None).expect("Failed to create Xlib surface") };
                println!("Vulkan surface created successfully (Linux X11)");
            }
            #[cfg(target_os = "linux")]
            RawWindowHandle::Wayland(handle) => {
                let display_handle = self.window.as_ref().unwrap().display_handle().expect("Failed to get display handle");
                let wayland_display_handle = match display_handle.as_raw() {
                    RawDisplayHandle::Wayland(wayland) => wayland,
                    _ => panic!("Expected Wayland display handle for Wayland window"),
                };
                let display = wayland_display_handle.display.as_ptr();
                let surface = handle.surface.as_ptr(); // Get surface from RawWindowHandle::Wayland
                let surface_create_info = vk::WaylandSurfaceCreateInfoKHR {
                    display,
                    surface,
                    ..Default::default()
                };
                let wayland_surface_instance = ash::khr::wayland_surface::Instance::new(&self.entry, self.instance.as_ref().unwrap());
                self.surface = unsafe { wayland_surface_instance.create_wayland_surface(&surface_create_info, None).expect("Failed to create Wayland surface") };
                println!("Vulkan surface created successfully (Linux Wayland)");
            }
            _ => panic!("Unsupported platform."),
        }

        // Physical device enumeration
        let physical_devices = unsafe {
            self.instance
                .as_ref()
                .unwrap()
                .enumerate_physical_devices()
                .expect("Failed to enumerate physical devices")
        };
        println!("Found {} physical devices", physical_devices.len());
        self.physical_device = physical_devices[0]; // Pick the first one for now
        println!("Selected physical device: {:?}", self.physical_device);

        // Queue family selection and device creation
        let queue_family_properties = unsafe {
            self.instance
                .as_ref()
                .unwrap()
                .get_physical_device_queue_family_properties(self.physical_device)
        };
        println!("Found {} queue families", queue_family_properties.len());
        let queue_family_index = queue_family_properties
            .iter()
            .position(|props| props.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .expect("No graphics queue family found") as u32;
        println!("Selected queue family index: {}", queue_family_index);

        let device_extension_names = vec![CString::new("VK_KHR_swapchain").unwrap()];
        let device_extension_names_ptrs: Vec<*const std::os::raw::c_char> =
            device_extension_names.iter().map(|c| c.as_ptr()).collect();
        let device_create_info = vk::DeviceCreateInfo {
            queue_create_info_count: 1,
            p_queue_create_infos: &vk::DeviceQueueCreateInfo {
                queue_family_index,
                queue_count: 1,
                p_queue_priorities: &1.0,
                ..Default::default()
            },
            enabled_extension_count: device_extension_names_ptrs.len() as u32,
            pp_enabled_extension_names: device_extension_names_ptrs.as_ptr(),
            ..Default::default()
        };
        self.device = Some(unsafe {
            self.instance
                .as_ref()
                .unwrap()
                .create_device(self.physical_device, &device_create_info, None)
                .expect("Failed to create Vulkan device")
        });
        println!("Vulkan device created successfully");
        self.queue = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .get_device_queue(queue_family_index, 0)
        };
        println!("Graphics queue obtained: {:?}", self.queue);

        // Swapchain creation
        let surface_instance =
            ash::khr::surface::Instance::new(&self.entry, self.instance.as_ref().unwrap());
        let surface_capabilities = unsafe {
            surface_instance
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
                .expect("Failed to get surface capabilities")
        };
        let surface_formats = unsafe {
            surface_instance
                .get_physical_device_surface_formats(self.physical_device, self.surface)
                .expect("Failed to get surface formats")
        };
        let present_modes = unsafe {
            surface_instance
                .get_physical_device_surface_present_modes(self.physical_device, self.surface)
                .expect("Failed to get present modes")
        };
        println!("Surface formats: {:?}", surface_formats);
        println!("Present modes: {:?}", present_modes);

        let format = surface_formats[0];
        let present_mode = present_modes
            .into_iter()
            .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::IMMEDIATE);
        let extent = if surface_capabilities.current_extent.width == u32::MAX {
            let window_size = window.inner_size();
            vk::Extent2D {
                width: window_size.width,
                height: window_size.height,
            }
        } else {
            surface_capabilities.current_extent
        };
        let image_count = surface_capabilities.min_image_count + 1;
        let image_count = if surface_capabilities.max_image_count > 0 {
            image_count.min(surface_capabilities.max_image_count)
        } else {
            image_count
        };

        let swapchain_create_info = vk::SwapchainCreateInfoKHR {
            surface: self.surface,
            min_image_count: image_count,
            image_format: format.format,
            image_color_space: format.color_space,
            image_extent: extent,
            image_array_layers: 1,
            image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            pre_transform: surface_capabilities.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
            present_mode,
            clipped: vk::TRUE,
            ..Default::default()
        };
        self.swapchain_ext = Some(ash::khr::swapchain::Device::new(
            self.instance.as_ref().unwrap(),
            self.device.as_ref().unwrap(),
        ));
        self.swapchain = unsafe {
            self.swapchain_ext
                .as_ref()
                .unwrap()
                .create_swapchain(&swapchain_create_info, None)
                .expect("Failed to create swapchain")
        };
        println!("Swapchain created: {:?}", self.swapchain);
        self.images = unsafe {
            self.swapchain_ext
                .as_ref()
                .unwrap()
                .get_swapchain_images(self.swapchain)
                .expect("Failed to get swapchain images")
        };
        println!("Swapchain images obtained: {:?}", self.images);

        // Image views creation
        self.image_views = self
            .images
            .iter()
            .map(|&image| {
                let create_info = vk::ImageViewCreateInfo {
                    image,
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: format.format,
                    components: vk::ComponentMapping::default(),
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    ..Default::default()
                };
                unsafe {
                    self.device
                        .as_ref()
                        .unwrap()
                        .create_image_view(&create_info, None)
                        .expect("Failed to create image view")
                }
            })
            .collect();
        println!("Image views created: {:?}", self.image_views);

        // Render pass creation
        let attachment = vk::AttachmentDescription {
            format: format.format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        };
        let color_attachment_ref = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        };
        let subpass = vk::SubpassDescription {
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachment_count: 1,
            p_color_attachments: &color_attachment_ref,
            ..Default::default()
        };
        let render_pass_create_info = vk::RenderPassCreateInfo {
            attachment_count: 1,
            p_attachments: &attachment,
            subpass_count: 1,
            p_subpasses: &subpass,
            ..Default::default()
        };
        self.render_pass = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .create_render_pass(&render_pass_create_info, None)
                .expect("Failed to create render pass")
        };
        println!("Render pass created: {:?}", self.render_pass);

        // Framebuffers creation
        self.framebuffers = self
            .image_views
            .iter()
            .map(|&image_view| {
                let framebuffer_create_info = vk::FramebufferCreateInfo {
                    render_pass: self.render_pass,
                    attachment_count: 1,
                    p_attachments: &image_view,
                    width: extent.width,
                    height: extent.height,
                    layers: 1,
                    ..Default::default()
                };
                unsafe {
                    self.device
                        .as_ref()
                        .unwrap()
                        .create_framebuffer(&framebuffer_create_info, None)
                        .expect("Failed to create framebuffer")
                }
            })
            .collect();
        println!("Framebuffers created: {:?}", self.framebuffers);

        // Command pool creation
        let command_pool_create_info = vk::CommandPoolCreateInfo {
            queue_family_index,
            ..Default::default()
        };
        self.command_pool = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .create_command_pool(&command_pool_create_info, None)
                .expect("Failed to create command pool")
        };
        println!("Command pool created: {:?}", self.command_pool);

        // Command buffer allocation
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo {
            s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
            p_next: std::ptr::null(),
            _marker: std::marker::PhantomData,
            command_pool: self.command_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: 1,
        };
        self.command_buffer = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .allocate_command_buffers(&command_buffer_allocate_info)
                .expect("Failed to allocate command buffers")[0]
        };
        println!("Command buffer allocated: {:?}", self.command_buffer);

        // Semaphore creation
        self.image_available_semaphore = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                .expect("Failed to create image available semaphore")
        };
        println!(
            "Image available semaphore created: {:?}",
            self.image_available_semaphore
        );
        self.render_finished_semaphore = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                .expect("Failed to create render finished semaphore")
        };
        println!(
            "Render finished semaphore created: {:?}",
            self.render_finished_semaphore
        );

        // Vertex buffer creation
        let vertices = create_circle_vertices(50.0, 32);
        self.create_vertex_buffer(&vertices);

        // Graphics pipeline creation
        self.create_graphics_pipeline();

        // Set extent (move this after swapchain creation, before image views)
        self.extent = extent;

        // Initialize circle position and velocity
        self.circle_position = Vec2::new(
            self.extent.width as f32 / 2.0,
            self.extent.height as f32 / 2.0,
        );
        self.circle_velocity = Vec2::new(200.0, 150.0); // pixels per second
        self.window.as_ref().unwrap().request_redraw();
    }

    fn create_vertex_buffer(&mut self, vertices: &[Vertex]) {
        let buffer_size = size_of_val(vertices) as vk::DeviceSize;
        let buffer_create_info = vk::BufferCreateInfo {
            size: buffer_size,
            usage: vk::BufferUsageFlags::VERTEX_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        self.vertex_buffer = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .create_buffer(&buffer_create_info, None)
                .expect("Failed to create vertex buffer")
        };

        let mem_requirements = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .get_buffer_memory_requirements(self.vertex_buffer)
        };

        let memory_type_index = self.find_memory_type(
            mem_requirements.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );

        let alloc_info = vk::MemoryAllocateInfo {
            allocation_size: mem_requirements.size,
            memory_type_index,
            ..Default::default()
        };

        self.vertex_buffer_memory = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .allocate_memory(&alloc_info, None)
                .expect("Failed to allocate vertex buffer memory")
        };

        unsafe {
            self.device
                .as_ref()
                .unwrap()
                .bind_buffer_memory(self.vertex_buffer, self.vertex_buffer_memory, 0)
                .expect("Failed to bind vertex buffer memory");

            let data_ptr = self
                .device
                .as_ref()
                .unwrap()
                .map_memory(
                    self.vertex_buffer_memory,
                    0,
                    buffer_size,
                    vk::MemoryMapFlags::empty(),
                )
                .expect("Failed to map memory") as *mut Vertex;
            data_ptr.copy_from_nonoverlapping(vertices.as_ptr(), vertices.len());
            self.device
                .as_ref()
                .unwrap()
                .unmap_memory(self.vertex_buffer_memory);
        }
        println!("Vertex buffer created: {:?}", self.vertex_buffer);
    }

    fn create_graphics_pipeline(&mut self) {
        let vertex_shader_code = include_bytes!("../shaders/vert.spv");
        let vertex_shader_module = self.create_shader_module(vertex_shader_code);

        let fragment_shader_code = include_bytes!("../shaders/frag.spv");
        let fragment_shader_module = self.create_shader_module(fragment_shader_code);

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo {
            vertex_binding_description_count: 1,
            p_vertex_binding_descriptions: &vk::VertexInputBindingDescription {
                binding: 0,
                stride: size_of::<Vertex>() as u32,
                input_rate: vk::VertexInputRate::VERTEX,
            },
            vertex_attribute_description_count: 1,
            p_vertex_attribute_descriptions: &vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: 0,
            },
            ..Default::default()
        };

        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo {
            push_constant_range_count: 1,
            p_push_constant_ranges: &vk::PushConstantRange {
                stage_flags: vk::ShaderStageFlags::VERTEX,
                offset: 0,
                size: std::mem::size_of::<Mat4>() as u32,
            },
            ..Default::default()
        };
        self.pipeline_layout = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .create_pipeline_layout(&pipeline_layout_create_info, None)
                .expect("Failed to create pipeline layout")
        };

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::VERTEX,
                module: vertex_shader_module,
                p_name: b"main\0".as_ptr() as *const _,
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                stage: vk::ShaderStageFlags::FRAGMENT,
                module: fragment_shader_module,
                p_name: b"main\0".as_ptr() as *const _,
                ..Default::default()
            },
        ];

        let pipeline_info = vk::GraphicsPipelineCreateInfo {
            stage_count: 2,
            p_stages: shader_stages.as_ptr(),
            p_vertex_input_state: &vertex_input_info,
            p_input_assembly_state: &vk::PipelineInputAssemblyStateCreateInfo {
                topology: vk::PrimitiveTopology::TRIANGLE_FAN,
                ..Default::default()
            },
            p_viewport_state: &vk::PipelineViewportStateCreateInfo {
                viewport_count: 1,
                scissor_count: 1,
                ..Default::default()
            },
            p_rasterization_state: &vk::PipelineRasterizationStateCreateInfo {
                polygon_mode: vk::PolygonMode::FILL,
                line_width: 1.0,
                cull_mode: vk::CullModeFlags::NONE,
                front_face: vk::FrontFace::CLOCKWISE,
                ..Default::default()
            },
            p_multisample_state: &vk::PipelineMultisampleStateCreateInfo {
                rasterization_samples: vk::SampleCountFlags::TYPE_1,
                ..Default::default()
            },
            p_color_blend_state: &vk::PipelineColorBlendStateCreateInfo {
                attachment_count: 1,
                p_attachments: &vk::PipelineColorBlendAttachmentState {
                    blend_enable: vk::FALSE,
                    color_write_mask: vk::ColorComponentFlags::RGBA,
                    ..Default::default()
                },
                ..Default::default()
            },
            p_dynamic_state: &vk::PipelineDynamicStateCreateInfo {
                dynamic_state_count: 2,
                p_dynamic_states: [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR].as_ptr(),
                ..Default::default()
            },
            layout: self.pipeline_layout,
            render_pass: self.render_pass,
            subpass: 0,
            ..Default::default()
        };

        self.pipeline = unsafe {
            self.device
                .as_ref()
                .unwrap()
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .expect("Failed to create graphics pipeline")[0]
        };

        unsafe {
            self.device
                .as_ref()
                .unwrap()
                .destroy_shader_module(vertex_shader_module, None);
            self.device
                .as_ref()
                .unwrap()
                .destroy_shader_module(fragment_shader_module, None);
        }
        println!("Graphics pipeline created: {:?}", self.pipeline);
    }

    fn find_memory_type(&self, type_filter: u32, properties: vk::MemoryPropertyFlags) -> u32 {
        let mem_properties = unsafe {
            self.instance
                .as_ref()
                .unwrap()
                .get_physical_device_memory_properties(self.physical_device)
        };
        for i in 0..mem_properties.memory_type_count {
            if (type_filter & (1 << i)) != 0
                && (mem_properties.memory_types[i as usize].property_flags & properties)
                    == properties
            {
                return i;
            }
        }
        panic!("Failed to find suitable memory type");
    }

    fn create_shader_module(&self, code: &[u8]) -> vk::ShaderModule {
        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: code.as_ptr() as *const u32,
            ..Default::default()
        };
        unsafe {
            self.device
                .as_ref()
                .unwrap()
                .create_shader_module(&create_info, None)
                .expect("Failed to create shader module")
        }
    }

    fn update_circle_position(&mut self) {
        static mut LAST_TIME: Option<std::time::Instant> = None;
        let now = std::time::Instant::now();
        let dt = unsafe {
            LAST_TIME.map(|last| now.duration_since(last).as_secs_f32()).unwrap_or(1.0 / 60.0)
        };
        unsafe { LAST_TIME = Some(now); }

        self.circle_position += self.circle_velocity * dt;

        let radius = 50.0;
        let bounds = Vec2::new(self.extent.width as f32, self.extent.height as f32);

        if self.circle_position.x - radius < 0.0 || self.circle_position.x + radius > bounds.x {
            self.circle_velocity.x = -self.circle_velocity.x;
        }
        if self.circle_position.y - radius < 0.0 || self.circle_position.y + radius > bounds.y {
            self.circle_velocity.y = -self.circle_velocity.y;
        }
    }

    fn render(&mut self) {
        // Reset command buffer to prevent state corruption
        unsafe {
            self.device
                .as_ref()
                .unwrap()
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .expect("Failed to reset command buffer");
        }

        // Acquire the next swapchain image
        let result = unsafe {
            self.swapchain_ext.as_ref().unwrap().acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available_semaphore,
                vk::Fence::null(),
            )
        };

        let (image_index, _) = match result {
            Ok(index) => index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain();
                return;
            }
            Err(e) => panic!("Failed to acquire next image: {:?}", e),
        };

        // Begin command buffer recording
        unsafe {
            self.device
                .as_ref()
                .unwrap()
                .begin_command_buffer(self.command_buffer, &vk::CommandBufferBeginInfo::default())
                .expect("Failed to begin command buffer");

            // Start render pass with clear color (black)
            let clear_value = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            };
            let render_pass_begin_info = vk::RenderPassBeginInfo {
                render_pass: self.render_pass,
                framebuffer: self.framebuffers[image_index as usize],
                render_area: vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.extent,
                },
                clear_value_count: 1,
                p_clear_values: &clear_value,
                ..Default::default()
            };

            self.device.as_ref().unwrap().cmd_begin_render_pass(
                self.command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            // Bind graphics pipeline
            self.device.as_ref().unwrap().cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            // Set viewport and scissor
            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: self.extent.width as f32,
                height: self.extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            self.device
                .as_ref()
                .unwrap()
                .cmd_set_viewport(self.command_buffer, 0, &[viewport]);

            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.extent,
            };
            self.device
                .as_ref()
                .unwrap()
                .cmd_set_scissor(self.command_buffer, 0, &[scissor]);

            // Bind vertex buffer
            self.device.as_ref().unwrap().cmd_bind_vertex_buffers(
                self.command_buffer,
                0,
                &[self.vertex_buffer],
                &[0],
            );

            // Set up transformation matrix for circle position
            let ortho = Mat4::orthographic_rh(
                0.0,
                self.extent.width as f32,
                self.extent.height as f32,
                0.0,
                -1.0,
                1.0,
            );
            let transform = Mat4::from_translation(self.circle_position.extend(0.0));
            let mvp = ortho * transform;
            let mvp_array = mvp.to_cols_array();
            self.device.as_ref().unwrap().cmd_push_constants(
                self.command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytemuck::cast_slice(&mvp_array),
            );

            // Draw the circle (triangle fan, 32 segments + center + closing vertex)
            self.device.as_ref().unwrap().cmd_draw(
                self.command_buffer,
                34,
                1,
                0,
                0,
            );

            // End render pass and command buffer
            self.device
                .as_ref()
                .unwrap()
                .cmd_end_render_pass(self.command_buffer);
            self.device
                .as_ref()
                .unwrap()
                .end_command_buffer(self.command_buffer)
                .expect("Failed to end command buffer");

            // Submit commands to the queue
            let wait_semaphores = [self.image_available_semaphore];
            let signal_semaphores = [self.render_finished_semaphore];
            let submit_info = vk::SubmitInfo {
                wait_semaphore_count: 1,
                p_wait_semaphores: wait_semaphores.as_ptr(),
                p_wait_dst_stage_mask: &vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                command_buffer_count: 1,
                p_command_buffers: &self.command_buffer,
                signal_semaphore_count: 1,
                p_signal_semaphores: signal_semaphores.as_ptr(),
                ..Default::default()
            };
            self.device
                .as_ref()
                .unwrap()
                .queue_submit(self.queue, &[submit_info], vk::Fence::null())
                .expect("Failed to submit queue");

            // Present the rendered image
            let present_info = vk::PresentInfoKHR {
                wait_semaphore_count: 1,
                p_wait_semaphores: &self.render_finished_semaphore,
                swapchain_count: 1,
                p_swapchains: &self.swapchain,
                p_image_indices: &image_index,
                ..Default::default()
            };
            let present_result = self
                .swapchain_ext
                .as_ref()
                .unwrap()
                .queue_present(self.queue, &present_info);

            match present_result {
                Ok(_) => (),
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.recreate_swapchain();
                    return;
                }
                Err(e) => panic!("Failed to present queue: {:?}", e),
            }
        }

        // Calculate FPS and update window title every second
        self.frame_count += 1;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_title_update).as_secs_f32();
        if elapsed >= 1.0 {
            self.fps = self.frame_count as f32 / elapsed;
            self.window
                .as_ref()
                .unwrap()
                .set_title(&format!("Vulkan Vibe - FPS: {:.1}", self.fps));
            self.last_title_update = now;
            self.frame_count = 0;
        }

        // Request the next frame
        self.window.as_ref().unwrap().request_redraw();
    }

    fn recreate_swapchain(&mut self) {
        unsafe {
            self.device
                .as_ref()
                .unwrap()
                .device_wait_idle()
                .expect("Failed to wait for device idle");

            for &framebuffer in &self.framebuffers {
                self.device
                    .as_ref()
                    .unwrap()
                    .destroy_framebuffer(framebuffer, None);
            }
            for &image_view in &self.image_views {
                self.device
                    .as_ref()
                    .unwrap()
                    .destroy_image_view(image_view, None);
            }
            self.swapchain_ext
                .as_ref()
                .unwrap()
                .destroy_swapchain(self.swapchain, None);

            let window = self.window.as_ref().unwrap();
            let new_size = window.inner_size();
            self.extent = vk::Extent2D {
                width: new_size.width,
                height: new_size.height,
            };

            let surface_instance =
                ash::khr::surface::Instance::new(&self.entry, self.instance.as_ref().unwrap());
            let surface_capabilities = surface_instance
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
                .expect("Failed to get surface capabilities");
            let surface_formats = surface_instance
                .get_physical_device_surface_formats(self.physical_device, self.surface)
                .expect("Failed to get surface formats");
            let present_modes = surface_instance
                .get_physical_device_surface_present_modes(self.physical_device, self.surface)
                .expect("Failed to get present modes");

            let format = surface_formats[0];
            let present_mode = present_modes
                .into_iter()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::IMMEDIATE);
            let image_count = surface_capabilities.min_image_count + 1;
            let image_count = if surface_capabilities.max_image_count > 0 {
                image_count.min(surface_capabilities.max_image_count)
            } else {
                image_count
            };

            let swapchain_create_info = vk::SwapchainCreateInfoKHR {
                surface: self.surface,
                min_image_count: image_count,
                image_format: format.format,
                image_color_space: format.color_space,
                image_extent: self.extent,
                image_array_layers: 1,
                image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
                pre_transform: surface_capabilities.current_transform,
                composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
                present_mode,
                clipped: vk::TRUE,
                ..Default::default()
            };
            self.swapchain = self
                .swapchain_ext
                .as_ref()
                .unwrap()
                .create_swapchain(&swapchain_create_info, None)
                .expect("Failed to recreate swapchain");
            self.images = self
                .swapchain_ext
                .as_ref()
                .unwrap()
                .get_swapchain_images(self.swapchain)
                .expect("Failed to get swapchain images");

            self.image_views = self
                .images
                .iter()
                .map(|&image| {
                    let create_info = vk::ImageViewCreateInfo {
                        image,
                        view_type: vk::ImageViewType::TYPE_2D,
                        format: format.format,
                        components: vk::ComponentMapping::default(),
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                        ..Default::default()
                    };
                    self.device
                        .as_ref()
                        .unwrap()
                        .create_image_view(&create_info, None)
                        .expect("Failed to create image view")
                })
                .collect();

            self.framebuffers = self
                .image_views
                .iter()
                .map(|&image_view| {
                    let framebuffer_create_info = vk::FramebufferCreateInfo {
                        render_pass: self.render_pass,
                        attachment_count: 1,
                        p_attachments: &image_view,
                        width: self.extent.width,
                        height: self.extent.height,
                        layers: 1,
                        ..Default::default()
                    };
                    self.device
                        .as_ref()
                        .unwrap()
                        .create_framebuffer(&framebuffer_create_info, None)
                        .expect("Failed to create framebuffer")
                })
                .collect();
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    println!("Event loop created");

    let mut app = App {
        window: None,
        entry: unsafe { ash::Entry::load().expect("Failed to load Vulkan entry") },
        instance: None,
        surface: vk::SurfaceKHR::null(),
        physical_device: vk::PhysicalDevice::null(),
        device: None,
        queue: vk::Queue::null(),
        swapchain: vk::SwapchainKHR::null(),
        swapchain_ext: None,
        images: Vec::new(),
        image_views: Vec::new(),
        render_pass: vk::RenderPass::null(),
        framebuffers: Vec::new(),
        command_pool: vk::CommandPool::null(),
        command_buffer: vk::CommandBuffer::null(),
        image_available_semaphore: vk::Semaphore::null(),
        render_finished_semaphore: vk::Semaphore::null(),
        pipeline: vk::Pipeline::null(),
        pipeline_layout: vk::PipelineLayout::null(),
        vertex_buffer: vk::Buffer::null(),
        vertex_buffer_memory: vk::DeviceMemory::null(),
        extent: vk::Extent2D {
            width: 0,
            height: 0,
        },
        circle_position: Vec2::ZERO,
        circle_velocity: Vec2::ZERO,
        last_title_update: std::time::Instant::now(),
        frame_count: 0,
        fps: 0.0,
    };
    println!("App initialized with Vulkan entry");

    event_loop.run_app(&mut app).expect("Event loop run failed");
    println!("Application exited");
}
