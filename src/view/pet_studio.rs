use crate::services::attach::Attachment;
use crate::ui::pet_design::PetDesign;
use crate::view::*;

#[derive(Clone, Default)]
pub struct PetStudio {
    pub enabled: bool,
    pub text_only: bool,
    pub pending: bool,
    pub draft: Option<PetDesign>,
    pub files: Vec<Attachment>,
    pub error: Option<String>,
    pub applied: bool,
    pub last_prompt: String,
    pub from_companion: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PetStudioHit {
    Apply,
    Photo,
    TextOnly,
}

pub fn pet_studio_buttons(area: Rect, scale: f32) -> [Rect; 3] {
    let gap = 6.0 * scale;
    let width = (area.2 - area.0 - 2.0 * gap) / 3.0;
    std::array::from_fn(|i| {
        let x = area.0 + i as f32 * (width + gap);
        (x, area.3 - 30.0 * scale, x + width, area.3)
    })
}

pub fn hit_pet_studio(area: Rect, scale: f32, point: (f32, f32), studio: &PetStudio) -> Option<PetStudioHit> {
    if studio.pending {
        return None;
    }
    pet_studio_buttons(area, scale)
        .into_iter()
        .zip([PetStudioHit::Apply, PetStudioHit::Photo, PetStudioHit::TextOnly])
        .find_map(|(rect, action)| {
            (inside(rect, point.0, point.1)
                && (action != PetStudioHit::Apply || (studio.draft.is_some() && !studio.applied)))
                .then_some(action)
        })
}

pub fn paint_pet_studio(p: &Painter, area: Rect, scale: f32, studio: &PetStudio) -> Result<()> {
    let (l, t, r, b) = area;
    let size = (136.0 * scale).min((r - l) * 0.42).min((b - t - 42.0 * scale).max(44.0 * scale));
    let top = t + ((b - t - 42.0 * scale - size) / 2.0).max(0.0);
    let preview = (l, top, l + size, top + size);
    p.fill_round_rect(preview, 18.0 * scale, rgba(1.0, 1.0, 1.0, 0.045))?;
    let old = p.push_offset(l + size * 0.04, top + size * 0.15);
    let pose = crate::ui::characters::Pose {
        t: 0.8,
        reaction: crate::ui::characters::Reaction::Idle,
        look: (0.2, 0.0),
        blink: false,
        accent: crate::ui::theme::accent(),
        beat: 0.0,
        motion: Default::default(),
    };
    let drawn = crate::ui::characters::draw_with_design(
        p,
        crate::ui::characters::Character::Custom,
        size * 0.92,
        &pose,
        studio.draft.as_ref(),
    );
    p.set_transform(&old);
    drawn?;
    let x = preview.2 + 12.0 * scale;
    let t = top + ((size - 104.0 * scale) / 2.0).max(0.0);
    let title = if studio.pending {
        "Creating your pet…"
    } else {
        studio.draft.as_ref().map_or("Create your pet", |pet| pet.name.as_str())
    };
    p.text(
        title,
        (x, t, r, t + 23.0 * scale),
        14.0 * scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        WHITE,
    )?;
    let hint = if studio.pending {
        "Building a preview. Your current pet stays with you."
    } else if studio.applied {
        "Your pet is ready. Describe another change below."
    } else if studio.draft.is_some() {
        "Preview ready. Apply your pet or describe another change below."
    } else if studio.text_only {
        "Describe the shape, colors and details. Photos will not be sent."
    } else if studio.files.is_empty() {
        "Describe your pet or attach a photo. A model with vision is needed for photos."
    } else {
        "Photo attached. Add your wishes below, then press Send."
    };
    let error = studio.error.as_deref();
    p.text_wrapped_trimmed(
        &crate::ui::i18n::tr(error.unwrap_or(hint)),
        (x, t + 27.0 * scale, r, (b - 38.0 * scale).min(t + 108.0 * scale)),
        11.5 * scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        if error.is_some() { rgba(1.0, 0.62, 0.55, 1.0) } else { rgba(1.0, 1.0, 1.0, 0.63) },
    )?;
    for (i, rect) in pet_studio_buttons(area, scale).into_iter().enumerate() {
        let active = if i == 0 { studio.draft.is_some() && !studio.applied } else { true } && !studio.pending;
        let selected = (i == 1 && !studio.files.is_empty() && !studio.text_only) || (i == 2 && studio.text_only);
        p.fill_round_rect(
            rect,
            10.0 * scale,
            if i == 0 || selected { acc(if active { 0.2 } else { 0.06 }) } else { rgba(1.0, 1.0, 1.0, 0.07) },
        )?;
        let label = match i {
            0 if studio.applied => "Applied",
            0 => "Apply pet",
            1 => "Pet photo",
            _ => "Text only",
        };
        p.text(
            label,
            rect,
            11.5 * scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, if active { 0.94 } else { 0.4 }),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_never_applies_without_an_explicit_preview_action() {
        let mut studio = PetStudio::default();
        for scale in [0.75, 1.0, 1.5, 2.0] {
            let area = (20.0 * scale, 50.0 * scale, 380.0 * scale, 260.0 * scale);
            let [apply, photo, text] = pet_studio_buttons(area, scale);
            assert!(apply.2 < photo.0 && photo.2 < text.0);
            let point = ((apply.0 + apply.2) / 2.0, (apply.1 + apply.3) / 2.0);
            studio.draft = None;
            assert_eq!(hit_pet_studio(area, scale, point, &studio), None);
            studio.draft = Some(Default::default());
            assert_eq!(hit_pet_studio(area, scale, point, &studio), Some(PetStudioHit::Apply));
            studio.pending = true;
            assert_eq!(hit_pet_studio(area, scale, point, &studio), None);
            studio.pending = false;
        }
    }
}
