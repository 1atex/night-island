use super::*;

use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
    RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
};
use windows::core::Error;

#[test]
#[ignore = "Requires an interactive Windows desktop and an isolated Composition runtime"]
fn weather_updates_a_region_and_recovers_after_a_failed_draw() -> Result<()> {
    struct Runtime;
    impl Drop for Runtime {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    unsafe { RoInitialize(RO_INIT_SINGLETHREADED)? };
    let _runtime = Runtime;
    let _queue = unsafe {
        CreateDispatcherQueueController(DispatcherQueueOptions {
            dwSize: size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_NONE,
        })?
    };
    let compositor = Compositor::new()?;
    let gpu = Gpu::new(&compositor)?;
    let layer = gpu.layer(&compositor, (160, 48))?;
    layer.paint(&gpu, |p| {
        p.fill_rect((0.0, 0.0, 160.0, 48.0), WHITE).unwrap();
    })?;
    let rect = (18.25, 10.75, 39.25, 31.75);
    layer.paint_region(&gpu, rect, |p| p.weather_svg("rain", rect, true))?;
    let failed = layer.paint_region(&gpu, rect, |_| Err(Error::from_hresult(E_INVALIDARG)));
    assert!(failed.is_err());
    layer.paint_region(&gpu, rect, |p| p.weather_svg("snow", rect, true))?;
    layer.paint_region(&gpu, (-20.0, -20.0, -10.0, -10.0), |_| panic!("an empty region must not draw"))?;
    layer.paint_region(&gpu, (150.5, 40.5, 180.0, 60.0), |p| p.fill_rect((150.0, 40.0, 180.0, 60.0), WHITE))?;
    Ok(())
}
