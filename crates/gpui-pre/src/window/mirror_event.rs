use super::*;
use crate::{
    LongPressEvent, MouseDownEvent, MouseExitEvent, MouseMoveEvent, MousePressureEvent,
    MouseUpEvent, PinchEvent, ScrollWheelEvent, TouchDragEvent,
};

impl Window {
    pub(crate) fn mirror_event(&self, event: &dyn Any) -> Option<PlatformInput> {
        if !self.mirror.enabled {
            return None;
        }
        let input = if let Some(event) = event.downcast_ref::<MouseMoveEvent>() {
            PlatformInput::MouseMove(event.clone())
        } else if let Some(event) = event.downcast_ref::<MouseDownEvent>() {
            PlatformInput::MouseDown(event.clone())
        } else if let Some(event) = event.downcast_ref::<MouseUpEvent>() {
            PlatformInput::MouseUp(event.clone())
        } else if let Some(event) = event.downcast_ref::<MouseExitEvent>() {
            PlatformInput::MouseExited(event.clone())
        } else if let Some(event) = event.downcast_ref::<MousePressureEvent>() {
            PlatformInput::MousePressure(event.clone())
        } else if let Some(event) = event.downcast_ref::<ScrollWheelEvent>() {
            PlatformInput::ScrollWheel(event.clone())
        } else if let Some(event) = event.downcast_ref::<PinchEvent>() {
            PlatformInput::Pinch(event.clone())
        } else if let Some(event) = event.downcast_ref::<LongPressEvent>() {
            PlatformInput::LongPress(event.clone())
        } else if let Some(event) = event.downcast_ref::<TouchDragEvent>() {
            PlatformInput::TouchDrag(event.clone())
        } else {
            return None;
        };
        let scroll = matches!(input, PlatformInput::ScrollWheel(_));
        let logical = |position| self.mirror_coordinates(position, self.mirror.event_scope, scroll);
        let map = |position| logical(position).0;
        Some(match input {
            PlatformInput::MouseMove(mut event) => {
                event.position = map(event.position);
                PlatformInput::MouseMove(event)
            }
            PlatformInput::MouseDown(mut event) => {
                event.position = map(event.position);
                PlatformInput::MouseDown(event)
            }
            PlatformInput::MouseUp(mut event) => {
                event.position = map(event.position);
                PlatformInput::MouseUp(event)
            }
            PlatformInput::MouseExited(mut event) => {
                event.position = map(event.position);
                PlatformInput::MouseExited(event)
            }
            PlatformInput::MousePressure(mut event) => {
                event.position = map(event.position);
                PlatformInput::MousePressure(event)
            }
            PlatformInput::ScrollWheel(mut event) => {
                let (position, in_region) = logical(event.position);
                event.position = position;
                if !in_region {
                    event.delta = Mirror::scroll_delta(event.delta);
                }
                PlatformInput::ScrollWheel(event)
            }
            PlatformInput::Pinch(mut event) => {
                event.position = map(event.position);
                PlatformInput::Pinch(event)
            }
            PlatformInput::LongPress(mut event) => {
                event.start_position = map(event.start_position);
                event.position = map(event.position);
                PlatformInput::LongPress(event)
            }
            PlatformInput::TouchDrag(mut event) => {
                event.start_position = map(event.start_position);
                event.position = map(event.position);
                PlatformInput::TouchDrag(event)
            }
            input => input,
        })
    }
}
