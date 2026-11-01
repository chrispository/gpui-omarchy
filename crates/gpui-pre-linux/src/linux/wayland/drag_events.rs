use super::drag::{DragState, NativeOffer};
use crate::linux::{LinuxCommon, transfer};
use gpui::{
    AnyWindowHandle, FileDropEvent, Modifiers, NativeDragAction, NativeDropEvent, Pixels, Point,
};
use std::{cell::RefCell, rc::Rc};
use wayland_client::{WEnum, protocol::wl_data_device_manager::DndAction};

/// Borrows the destination only for state access, never across an application callback.
pub(crate) trait DropHost {
    type Offer: NativeOffer + Clone;
    type Window: Clone;

    fn with_drag<R>(
        &self,
        f: impl FnOnce(
            &mut DragState<Self::Offer, Self::Window>,
            Modifiers,
            &Rc<RefCell<LinuxCommon>>,
        ) -> R,
    ) -> Option<R>;
    fn handle(window: &Self::Window) -> AnyWindowHandle;
    fn input(window: &Self::Window, event: FileDropEvent);
}

pub(crate) fn motion<H: DropHost>(host: &H, position: Point<Pixels>) {
    let Some((generation, entered)) = host.with_drag(|drag, _, _| {
        drag.position = position;
        (drag.generation, drag.entered && !drag.dropped)
    }) else {
        return;
    };
    if entered {
        dispatch(host);
        pending(host, generation);
    }
}

fn pending<H: DropHost>(host: &H, generation: u64) {
    let Some(Some((window, position))) = host.with_drag(|drag, _, _| {
        (drag.generation == generation && drag.entered && !drag.dropped)
            .then(|| drag.window.clone().map(|window| (window, drag.position)))
            .flatten()
    }) else {
        return;
    };
    H::input(&window, FileDropEvent::Pending { position });
}

pub(crate) fn action<H: DropHost>(
    host: &H,
    matches: impl FnOnce(&H::Offer) -> bool,
    dnd_action: WEnum<DndAction>,
) {
    let Some(Some((generation, entered))) = host.with_drag(|drag, _, _| {
        if !drag.offer.as_ref().is_some_and(matches) {
            return None;
        }
        drag.action = match dnd_action {
            WEnum::Value(DndAction::Copy) => Some(NativeDragAction::Copy),
            WEnum::Value(DndAction::Move) => Some(NativeDragAction::Move),
            _ => None,
        };
        Some((drag.generation, drag.entered && !drag.dropped))
    }) else {
        return;
    };
    dispatch(host);
    if entered {
        pending(host, generation);
    }
}

pub(crate) fn dispatch<H: DropHost>(host: &H) {
    let Some(Some((offer, window, mut event, generation, entered, dropped, paths, common))) = host
        .with_drag(|drag, modifiers, common| {
            let (Some(offer), Some(window), Some(data)) =
                (drag.offer.clone(), drag.window.clone(), drag.data.clone())
            else {
                return None;
            };
            let event = NativeDropEvent {
                window: H::handle(&window),
                position: drag.position,
                dropped: false,
                offered_types: drag.offered_types.clone(),
                data,
                action: drag.action,
                modifiers,
            };
            Some((
                offer,
                window,
                event,
                drag.generation,
                drag.entered,
                drag.dropped,
                drag.paths.clone(),
                common.clone(),
            ))
        })
    else {
        return;
    };
    let has_paths = !paths.0.is_empty();
    if !entered {
        let allowed = transfer::allow_drop(&common, event.clone(), has_paths);
        if host.with_drag(|drag, _, _| drag.admit(generation, allowed)) != Some(true) {
            return;
        }
        if has_paths {
            H::input(
                &window,
                FileDropEvent::Entered {
                    position: event.position,
                    paths,
                },
            );
        }
    } else if !dropped {
        // Refresh observation only; admission was decided at Enter and submission is decided at Drop.
        transfer::allow_drop(&common, event.clone(), has_paths);
    }
    if host.with_drag(|drag, _, _| drag.generation == generation) != Some(true) {
        return;
    }
    if !dropped {
        return;
    }
    let accepted = (offer.version() < 3 || event.action.is_some()) && {
        event.dropped = true;
        transfer::allow_drop(&common, event.clone(), has_paths)
    };
    if host.with_drag(|drag, _, _| drag.generation == generation) != Some(true) {
        return;
    }
    if has_paths {
        H::input(
            &window,
            if accepted {
                FileDropEvent::Submit {
                    position: event.position,
                }
            } else {
                FileDropEvent::Exited
            },
        );
    }
    host.with_drag(|drag, _, _| drag.submit(generation, accepted));
}

#[cfg(test)]
mod tests;
