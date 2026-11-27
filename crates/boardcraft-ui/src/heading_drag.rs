use gpui_kit::{Bounds, Pixels, Point, SharedString, px};
use std::time::{Duration, Instant};

/// Frame-local, window-coordinate geometry supplied by the presentation owner.
#[derive(Clone)]
pub struct SectionBounds {
    id: SharedString,
    name: Bounds<Pixels>,
    heading: Bounds<Pixels>,
    section: Bounds<Pixels>,
}
impl SectionBounds {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: Bounds::default(),
            heading: Bounds::default(),
            section: Bounds::default(),
        }
    }
    pub fn set_name(&mut self, bounds: Bounds<Pixels>) {
        self.name = bounds;
    }
    pub fn set_heading(&mut self, bounds: Bounds<Pixels>) {
        self.heading = bounds;
    }
    pub fn set_section(&mut self, bounds: Bounds<Pixels>) {
        self.section = bounds;
    }
}

struct Candidate {
    id: SharedString,
    origin: Point<Pixels>,
    pointer: Point<Pixels>,
    active: bool,
}

/// The containing visit owns this state and discards it on navigation.
/// Compose with Kit Buttons: their focus and double-click behavior is unchanged below
/// threshold. The caller captures pointer events, consumes a release click when requested,
/// paints the cue/edge, and offers a keyboard menu alternative to dragging.
#[derive(Default)]
pub struct HeadingDrag {
    sections: Vec<SectionBounds>,
    viewport: Bounds<Pixels>,
    candidate: Option<Candidate>,
    suppression: Option<Instant>,
    generation: u64,
}
impl HeadingDrag {
    pub fn new(ids: impl IntoIterator<Item = SharedString>) -> Self {
        Self {
            sections: ids.into_iter().map(SectionBounds::new).collect(),
            ..Self::default()
        }
    }
    pub fn section_mut(&mut self, id: &str) -> Option<&mut SectionBounds> {
        self.sections.iter_mut().find(|s| s.id.as_ref() == id)
    }
    pub fn set_viewport(&mut self, bounds: Bounds<Pixels>) {
        self.viewport = bounds;
    }
    pub fn surface_origin(&self) -> Point<Pixels> {
        self.sections
            .first()
            .map(|s| s.section.origin)
            .unwrap_or_default()
    }
    pub fn is_held(&self) -> bool {
        self.candidate.is_some()
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn is_active(&self) -> bool {
        self.candidate.as_ref().is_some_and(|c| c.active)
    }
    pub fn moving_id(&self) -> Option<&str> {
        self.candidate
            .as_ref()
            .filter(|c| c.active)
            .map(|c| c.id.as_ref())
    }
    pub fn source_id(&self) -> Option<&str> {
        self.candidate.as_ref().map(|c| c.id.as_ref())
    }
    pub fn pointer(&self) -> Option<Point<Pixels>> {
        self.candidate
            .as_ref()
            .filter(|c| c.active)
            .map(|c| c.pointer)
    }
    /// Only call for a primary, non-touch press on an enabled idle surface.
    pub fn press(&mut self, pointer: Point<Pixels>) {
        if self.candidate.is_some() || !self.viewport.contains(&pointer) {
            return;
        }
        if let Some(s) = self.sections.iter().find(|s| s.name.contains(&pointer)) {
            self.generation = self.generation.wrapping_add(1);
            self.candidate = Some(Candidate {
                id: s.id.clone(),
                origin: pointer,
                pointer,
                active: false,
            });
        }
    }
    pub fn move_pointer(&mut self, pointer: Point<Pixels>) {
        if let Some(c) = &mut self.candidate {
            c.pointer = pointer;
            let delta = pointer - c.origin;
            // Compared logical-pixel hit tolerance, independent of interface typography.
            c.active |= f32::from(delta.x).powi(2) + f32::from(delta.y).powi(2) >= 36.;
        }
    }
    pub fn edge(&self) -> Option<(&str, bool)> {
        let c = self.candidate.as_ref().filter(|c| c.active)?;
        if !self.viewport.contains(&c.pointer) {
            return None;
        }
        let s = self
            .sections
            .iter()
            .find(|s| s.section.contains(&c.pointer))?;
        if s.id == c.id {
            return None;
        }
        Some((s.id.as_ref(), c.pointer.y > s.heading.center().y))
    }
    pub fn marker(&self) -> Option<Bounds<Pixels>> {
        let (id, after) = self.edge()?;
        let s = self.sections.iter().find(|s| s.id.as_ref() == id)?;
        let mut bounds = s.heading;
        bounds.origin.y = if after { bounds.bottom() } else { bounds.top() };
        // A physical insertion hairline, not product spacing.
        bounds.size.height = px(1.);
        Some(bounds)
    }
    /// Logical pixels per second. The caller advances the existing scroll owner and
    /// refreshes measured geometry before the next target calculation.
    pub fn scroll_velocity(&self) -> f32 {
        let Some(p) = self.pointer().filter(|p| self.viewport.contains(p)) else {
            return 0.;
        };
        // Compared 44 logical-pixel edge band, with increasing speed toward the edge.
        let top = f32::from(p.y - self.viewport.top());
        let bottom = f32::from(self.viewport.bottom() - p.y);
        if top < 44. {
            -480. * (1. - top / 44.)
        } else if bottom < 44. {
            480. * (1. - bottom / 44.)
        } else {
            0.
        }
    }
    pub fn release(&mut self, pointer: Point<Pixels>) -> Option<Vec<SharedString>> {
        // A release is not a movement: an ordinary click must never cross threshold.
        if let Some(c) = &mut self.candidate {
            c.pointer = pointer;
        }
        let order = self.edge().and_then(|(target, after)| {
            let moving = &self.candidate.as_ref()?.id;
            let mut ids: Vec<_> = self
                .sections
                .iter()
                .filter(|s| &s.id != moving)
                .map(|s| s.id.clone())
                .collect();
            let ix = ids.iter().position(|id| id.as_ref() == target)? + usize::from(after);
            ids.insert(ix, moving.clone());
            (ids.iter().ne(self.sections.iter().map(|s| &s.id))).then_some(ids)
        });
        self.cancel();
        order
    }
    /// Escape, pointer loss, disabled state or visit departure: no order is emitted.
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.is_active() {
            self.suppression = Some(Instant::now() + Duration::from_millis(500));
        }
        self.candidate = None;
    }
    /// Consume only the first table click after an activated drag, including Escape.
    pub fn consume_click(&mut self) -> bool {
        self.suppression
            .take()
            .is_some_and(|until| Instant::now() <= until)
    }
}

#[cfg(test)]
mod tests {
    use super::{HeadingDrag, SectionBounds};
    use gpui_kit::{Bounds, point, px, size};
    fn fixture() -> HeadingDrag {
        let mut d = HeadingDrag::new(["a".into(), "b".into(), "c".into()]);
        d.set_viewport(Bounds::new(point(px(0.), px(0.)), size(px(200.), px(300.))));
        for (ix, id) in ["a", "b", "c"].iter().enumerate() {
            let s: &mut SectionBounds = d.section_mut(id).unwrap();
            let header = Bounds::new(point(px(0.), px(ix as f32 * 100.)), size(px(200.), px(20.)));
            s.set_name(header);
            s.set_heading(header);
            s.set_section(Bounds::new(header.origin, size(px(200.), px(100.))));
        }
        d
    }
    #[test]
    fn threshold_midpoint_noop_and_cancel() {
        let mut d = fixture();
        d.press(point(px(10.), px(210.)));
        d.move_pointer(point(px(13.), px(214.)));
        assert!(!d.is_active());
        d.move_pointer(point(px(16.), px(210.)));
        assert!(d.is_active());
        d.move_pointer(point(px(20.), px(10.)));
        assert_eq!(d.edge(), Some(("a", false)));
        d.move_pointer(point(px(20.), px(11.)));
        assert_eq!(d.edge(), Some(("a", true)));
        d.cancel();
        assert!(d.consume_click());
        assert!(!d.consume_click());
        d.press(point(px(10.), px(210.)));
        d.move_pointer(point(px(20.), px(10.)));
        assert_eq!(d.release(point(px(20.), px(10.))).unwrap(), ["c", "a", "b"]);
        d.press(point(px(10.), px(110.)));
        d.move_pointer(point(px(20.), px(80.)));
        assert!(d.release(point(px(20.), px(80.))).is_none());
        d.press(point(px(10.), px(210.)));
        d.move_pointer(point(px(20.), px(10.)));
        assert!(d.release(point(px(300.), px(10.))).is_none());
    }
    #[test]
    fn held_scroll_band_and_cleanup() {
        let mut d = fixture();
        d.press(point(px(10.), px(210.)));
        let generation = d.generation();
        d.move_pointer(point(px(10.), px(260.)));
        let slow = d.scroll_velocity();
        assert!(slow > 0.);
        d.move_pointer(point(px(10.), px(290.)));
        assert!(d.scroll_velocity() > slow);
        d.move_pointer(point(px(10.), px(150.)));
        assert_eq!(d.scroll_velocity(), 0.);
        d.move_pointer(point(px(10.), px(10.)));
        assert!(d.scroll_velocity() < 0.);
        d.cancel();
        assert_eq!(d.scroll_velocity(), 0.);
        assert_ne!(d.generation(), generation);
        assert!(d.marker().is_none());
        d.press(point(px(10.), px(210.)));
        d.move_pointer(point(px(13.), px(214.)));
        assert!(d.release(point(px(13.), px(214.))).is_none());
        assert!(!d.is_held());
    }
}
