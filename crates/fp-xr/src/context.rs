//! OpenXR instance and system, and Vulkan creation through the runtime.

use crate::{Error, Result, XrContextExt};
use ash::vk::{self, Handle};
use fp_xr_loader::{self as loader, RuntimeManifest};
use openxr as xr;

/// Defines `/interaction_profiles/valve/frame_controller_valve`. Not in the
/// Khronos registry yet; SteamVR on the Steam Frame provides it.
pub const FRAME_CONTROLLER_EXTENSION: &[u8] = b"XR_VALVE_frame_controller_interaction\0";

pub struct XrContext {
    pub instance: xr::Instance,
    pub system: xr::SystemId,
    pub manifest: RuntimeManifest,
    pub runtime: String,
    pub system_name: String,
    /// Extensions we enabled.
    pub enabled: xr::ExtensionSet,
    pub blend_modes: Vec<xr::EnvironmentBlendMode>,
}

impl XrContext {
    /// Loads the active runtime and creates an instance with the extensions
    /// FramePlayerX uses when they are available.
    pub fn new(app_name: &str) -> Result<XrContext> {
        let manifest = loader::find_active_runtime(&loader::SearchEnv::from_process())
            .map_err(Error::Runtime)?;
        let loaded = loader::load_runtime(&manifest).map_err(Error::Runtime)?;
        let entry = loaded.entry;
        let avail = entry.enumerate_extensions().ctx("enumerate extensions")?;
        if !avail.khr_vulkan_enable2 {
            return Err(Error::Runtime(
                "the OpenXR runtime lacks XR_KHR_vulkan_enable2".into(),
            ));
        }
        let mut e = xr::ExtensionSet::default();
        e.khr_vulkan_enable2 = true;
        e.fb_display_refresh_rate = avail.fb_display_refresh_rate;
        e.ext_eye_gaze_interaction = avail.ext_eye_gaze_interaction;
        e.ext_hand_interaction = avail.ext_hand_interaction;
        e.ext_hand_tracking = avail.ext_hand_tracking;
        e.khr_composition_layer_cylinder = avail.khr_composition_layer_cylinder;
        e.khr_composition_layer_equirect2 = avail.khr_composition_layer_equirect2;
        e.khr_visibility_mask = avail.khr_visibility_mask;
        // The Steam Frame controller profile only exists with this enabled.
        if avail
            .other
            .iter()
            .any(|n| n.as_slice() == FRAME_CONTROLLER_EXTENSION)
        {
            e.other.push(FRAME_CONTROLLER_EXTENSION.to_vec());
        }
        let mut instance = None;
        let mut last = None;
        for api in [xr::Version::new(1, 1, 0), xr::Version::new(1, 0, 0)] {
            let app = xr::ApplicationInfo {
                application_name: app_name,
                application_version: 1,
                engine_name: "frameplayerx",
                engine_version: 1,
                api_version: api,
            };
            match entry.create_instance(&app, &e, &[], &()) {
                Ok(i) => {
                    instance = Some(i);
                    break;
                }
                Err(err) => last = Some(err),
            }
        }
        let Some(instance) = instance else {
            return Err(Error::Xr(
                "xrCreateInstance",
                last.unwrap_or(xr::sys::Result::ERROR_RUNTIME_FAILURE),
            ));
        };
        let props = instance.properties().ctx("xrGetInstanceProperties")?;
        let system = instance
            .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
            .ctx("xrGetSystem (is the headset on?)")?;
        let sp = instance
            .system_properties(system)
            .ctx("xrGetSystemProperties")?;
        let blend_modes = instance
            .enumerate_environment_blend_modes(system, xr::ViewConfigurationType::PRIMARY_STEREO)
            .ctx("xrEnumerateEnvironmentBlendModes")?;
        log::info!(
            "OpenXR: {} {} on {}",
            props.runtime_name,
            props.runtime_version,
            sp.system_name
        );
        Ok(XrContext {
            instance,
            system,
            manifest,
            runtime: format!("{} {}", props.runtime_name, props.runtime_version),
            system_name: sp.system_name,
            enabled: e,
            blend_modes,
        })
    }

    /// Closes SteamVR's dashboard with the `vrcmd` tool beside the runtime
    /// library, without waiting for it. Does nothing on other runtimes.
    pub fn hide_steamvr_dashboard(&self) {
        if !self.manifest.is_steamvr {
            return;
        }
        let Some(dir) = std::path::Path::new(&self.manifest.resolved_library_path).parent() else {
            return;
        };
        let vrcmd = dir.join("vrcmd");
        // With Steam's variables SteamVR would file vrcmd under this app, and
        // the library path is the launcher's, for our own libraries.
        let child = std::process::Command::new(&vrcmd)
            .arg("--hidedashboard")
            .env_remove("SteamAppId")
            .env_remove("SteamGameId")
            .env_remove("SteamOverlayGameId")
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        match child {
            Ok(mut c) => {
                log::info!("closing the SteamVR dashboard");
                std::thread::spawn(move || c.wait());
            }
            Err(e) => log::warn!("{}: {e}", vrcmd.display()),
        }
    }

    /// Recommended per-eye render size.
    pub fn eye_size(&self) -> Result<(u32, u32)> {
        let views = self
            .instance
            .enumerate_view_configuration_views(
                self.system,
                xr::ViewConfigurationType::PRIMARY_STEREO,
            )
            .ctx("xrEnumerateViewConfigurationViews")?;
        let v = views
            .first()
            .ok_or_else(|| Error::Runtime("no views".into()))?;
        Ok((
            v.recommended_image_rect_width,
            v.recommended_image_rect_height,
        ))
    }

    /// A [`fp_render::Creator`] that lets the runtime create Vulkan objects.
    pub fn creator(&self) -> XrCreator<'_> {
        XrCreator { ctx: self }
    }
}

pub struct XrCreator<'a> {
    ctx: &'a XrContext,
}

impl fp_render::Creator for XrCreator<'_> {
    fn create_instance(
        &self,
        entry: &ash::Entry,
        info: &vk::InstanceCreateInfo,
    ) -> fp_render::Result<vk::Instance> {
        let x = self.ctx;
        x.instance
            .graphics_requirements::<xr::Vulkan>(x.system)
            .map_err(|e| {
                fp_render::Error::Unsupported(format!("XR graphics requirements: {e:?}"))
            })?;
        // SAFETY: the function pointer comes from the loaded Vulkan library;
        // the create info is valid for the duration of the call.
        let raw = unsafe {
            x.instance.create_vulkan_instance(
                x.system,
                std::mem::transmute::<
                    vk::PFN_vkGetInstanceProcAddr,
                    xr::sys::platform::VkGetInstanceProcAddr,
                >(entry.static_fn().get_instance_proc_addr),
                info as *const _ as *const _,
            )
        }
        .map_err(|e| fp_render::Error::Unsupported(format!("xrCreateVulkanInstanceKHR: {e:?}")))?
        .map_err(|r| fp_render::Error::Vk {
            context: "vkCreateInstance (via OpenXR)",
            result: vk::Result::from_raw(r),
        })?;
        Ok(vk::Instance::from_raw(raw as u64))
    }

    fn physical_device(&self, instance: &ash::Instance) -> fp_render::Result<vk::PhysicalDevice> {
        let x = self.ctx;
        // SAFETY: the instance was created through this runtime.
        let raw = unsafe {
            x.instance
                .vulkan_graphics_device(x.system, instance.handle().as_raw() as _)
        }
        .map_err(|e| {
            fp_render::Error::Unsupported(format!("xrGetVulkanGraphicsDevice2KHR: {e:?}"))
        })?;
        Ok(vk::PhysicalDevice::from_raw(raw as u64))
    }

    fn create_device(
        &self,
        instance: &ash::Instance,
        pdev: vk::PhysicalDevice,
        info: &vk::DeviceCreateInfo,
    ) -> fp_render::Result<vk::Device> {
        let x = self.ctx;
        // SAFETY: the static loader entry point and valid create info; see
        // create_instance.
        let entry =
            unsafe { ash::Entry::load() }.map_err(|e| fp_render::Error::Load(e.to_string()))?;
        let _ = instance;
        // SAFETY: as for create_vulkan_instance; pdev came from this
        // runtime's instance.
        let raw = unsafe {
            x.instance.create_vulkan_device(
                x.system,
                std::mem::transmute::<
                    vk::PFN_vkGetInstanceProcAddr,
                    xr::sys::platform::VkGetInstanceProcAddr,
                >(entry.static_fn().get_instance_proc_addr),
                pdev.as_raw() as _,
                info as *const _ as *const _,
            )
        }
        .map_err(|e| fp_render::Error::Unsupported(format!("xrCreateVulkanDeviceKHR: {e:?}")))?
        .map_err(|r| fp_render::Error::Vk {
            context: "vkCreateDevice (via OpenXR)",
            result: vk::Result::from_raw(r),
        })?;
        Ok(vk::Device::from_raw(raw as u64))
    }
}
