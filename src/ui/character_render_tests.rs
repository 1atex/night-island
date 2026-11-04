use super::*;
use crate::ui::gfx::Gpu;
use crate::ui::pet_design::{PetAccessory, PetBody, PetDesign};
use crate::ui::pet_motion::Motion;
use crate::ui::popup::PopupSurface;
use windows::UI::Composition::Compositor;
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
    RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
};

#[test]
#[ignore = "Requires an isolated Windows Composition runtime"]
fn moving_pets_and_custom_ears_fit_the_attached_canvas() -> Result<()> {
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
    let surface = PopupSurface::new((188, 156))?;
    let mut designs: Vec<_> =
        Character::ALL.into_iter().filter(|ch| *ch != Character::None).map(|ch| (ch, PetDesign::default())).collect();
    designs.extend(
        [PetBody::Bear, PetBody::Cat, PetBody::Fox, PetBody::Rabbit, PetBody::Ghost, PetBody::Robot].into_iter().map(
            |body| {
                (
                    Character::Custom,
                    PetDesign {
                        body,
                        width: 1.15,
                        ear_size: 1.25,
                        tail: true,
                        accessory: PetAccessory::Scarf,
                        ..Default::default()
                    },
                )
            },
        ),
    );
    let mut clipped = Vec::new();
    for (character, design) in designs {
        for frame in 0..12 {
            let pose = Pose {
                t: frame as f32 * 0.65,
                look: ((frame as f32).sin(), (frame as f32).cos()),
                reaction: [
                    Reaction::Idle,
                    Reaction::Music,
                    Reaction::Done,
                    Reaction::Attention,
                    Reaction::File,
                    Reaction::Sleep,
                ][frame % 6],
                beat: 1.0,
                blink: false,
                accent: (0.6, 0.7, 1.0),
                motion: Motion {
                    lean: if frame % 2 == 0 { 0.15 } else { -0.15 },
                    stretch: if frame % 2 == 0 { 0.1 } else { -0.08 },
                    ..Default::default()
                },
            };
            surface.paint(&gpu, |p| {
                let old = p.push_offset(14.0, 14.0);
                let result = draw_with_design(p, character, 160.0, &pose, Some(&design));
                p.set_transform(&old);
                result
            })?;
            let outside = surface
                .pixels()
                .chunks_exact(4)
                .enumerate()
                .filter(|(i, px)| {
                    let (x, y) = (i % 188, i / 188);
                    px[3] > 20 && (!(14..174).contains(&x) || !(14..134).contains(&y))
                })
                .count();
            if outside > 0 {
                clipped.push(format!("{character:?}/{:?} frame {frame}: {outside} pixels", design.body));
                break;
            }
        }
    }
    assert!(clipped.is_empty(), "{}", clipped.join("; "));
    if let Some(dir) = std::env::var_os("NI_COMPANION_PREVIEW") {
        let folder = std::path::PathBuf::from(dir).join("animation");
        std::fs::create_dir_all(&folder).unwrap();
        let frame_surface = PopupSurface::new((600, 154))?;
        let fox = PetDesign {
            name: "Лисёнок".into(),
            body: PetBody::Fox,
            fur: [223, 133, 66],
            belly: [249, 227, 195],
            accent: [112, 170, 222],
            tail: true,
            accessory: PetAccessory::Scarf,
            ..Default::default()
        };
        let mut motion = crate::ui::pet_motion::PetMotion::default();
        for frame in 0..120 {
            let t = frame as f32 / 30.0;
            let pose_motion = motion.advance(
                ((t * 3.0).sin() * 0.65, (t * 2.0).cos() * 0.35),
                1.0 / 30.0,
                frame < 72,
                (72..92).contains(&frame),
                false,
            );
            let pose = Pose {
                t,
                reaction: Reaction::Idle,
                look: motion.look,
                beat: 0.0,
                blink: false,
                accent: (0.6, 0.7, 1.0),
                motion: Motion {
                    lid: if (36..42).contains(&frame) {
                        crate::ui::pet_motion::eyelid((42 - frame) as f32 / 30.0)
                    } else {
                        0.0
                    },
                    ..pose_motion
                },
            };
            frame_surface.paint(&gpu, |p| {
                p.fill_rect((0.0, 0.0, 600.0, 154.0), rgba(0.055, 0.059, 0.075, 1.0))?;
                for (i, (ch, label)) in [
                    (Character::Spark, "Spark"),
                    (Character::Cat, "Кот"),
                    (Character::Bear, "Медведь"),
                    (Character::Custom, "Свой питомец"),
                ]
                .into_iter()
                .enumerate()
                {
                    let x = i as f32 * 150.0;
                    let old = p.push_offset(x + 15.0, 18.0);
                    let result = draw_with_design(p, ch, 120.0, &pose, Some(&fox));
                    p.set_transform(&old);
                    result?;
                    p.text(
                        label,
                        (x, 116.0, x + 150.0, 142.0),
                        13.0,
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                        rgba(0.9, 0.9, 0.94, 1.0),
                    )?;
                }
                Ok(())
            })?;
            let mut rgba = frame_surface.pixels();
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            let mut encoder =
                png::Encoder::new(std::fs::File::create(folder.join(format!("{frame:03}.png"))).unwrap(), 600, 154);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&rgba).unwrap();
        }
    }

    Ok(())
}
