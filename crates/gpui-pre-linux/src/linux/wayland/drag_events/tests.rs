use super::*;
use gpui::{ClipboardItem, Empty, MimePayload, MimeRepresentation, SelectionIntent, px};
use std::cell::Cell;

#[derive(Clone)]
struct Offer {
    id: u32,
    requests: Rc<RefCell<Vec<&'static str>>>,
}

impl NativeOffer for Offer {
    fn version(&self) -> u32 {
        3
    }
    fn accept(&self, _: u32, mime: Option<String>) {
        self.requests
            .borrow_mut()
            .push(if mime.is_some() { "accept" } else { "refuse" });
    }
    fn actions(&self, _: gpui::NativeDragActions) {
        self.requests.borrow_mut().push("actions");
    }
    fn finish(&self) {
        self.requests.borrow_mut().push("finish");
    }
}

#[derive(Clone)]
struct Target {
    handle: AnyWindowHandle,
    inputs: Rc<RefCell<Vec<FileDropEvent>>>,
    trace: Rc<RefCell<Vec<&'static str>>>,
    on_input: Rc<RefCell<Option<Box<dyn FnMut()>>>>,
}

#[derive(Clone)]
struct Host {
    drag: Rc<RefCell<DragState<Offer, Target>>>,
    modifiers: Rc<Cell<Modifiers>>,
    common: Rc<RefCell<LinuxCommon>>,
}

impl DropHost for Host {
    type Offer = Offer;
    type Window = Target;
    fn with_drag<R>(
        &self,
        f: impl FnOnce(&mut DragState<Offer, Target>, Modifiers, &Rc<RefCell<LinuxCommon>>) -> R,
    ) -> Option<R> {
        Some(f(
            &mut self.drag.borrow_mut(),
            self.modifiers.get(),
            &self.common,
        ))
    }
    fn handle(window: &Target) -> AnyWindowHandle {
        window.handle
    }
    fn input(window: &Target, event: FileDropEvent) {
        window.trace.borrow_mut().push(match &event {
            FileDropEvent::Entered { .. } => "enter",
            FileDropEvent::Pending { .. } => "pending",
            FileDropEvent::Submit { .. } => "submit",
            FileDropEvent::Exited => "exit",
            _ => panic!("unexpected file event"),
        });
        window.inputs.borrow_mut().push(event);
        let callback = window.on_input.borrow_mut().take();
        if let Some(mut callback) = callback {
            callback();
        }
    }
}

struct Fixture {
    host: Host,
    target: Target,
    offer: Offer,
    events: Rc<RefCell<Vec<NativeDropEvent>>>,
    generation: u64,
}

impl Fixture {
    fn new(cx: &mut gpui::TestAppContext, files: bool) -> Self {
        let window = cx.add_window(|_, _| Empty);
        let target = Target {
            handle: window.into(),
            inputs: Rc::default(),
            trace: Rc::default(),
            on_input: Rc::default(),
        };
        let event_loop = calloop::EventLoop::<()>::try_new().unwrap();
        let (common, _, _) = LinuxCommon::new(event_loop.get_signal());
        let host = Host {
            drag: Rc::default(),
            modifiers: Rc::default(),
            common: Rc::new(RefCell::new(common)),
        };
        let events = Rc::new(RefCell::new(Vec::new()));
        host.common.borrow_mut().callbacks.native_drop = Some(Box::new({
            let events = events.clone();
            let trace = target.trace.clone();
            let drag = host.drag.clone();
            move |event| {
                // Both the host and callback registry must be unborrowed during delivery.
                assert!(drag.try_borrow_mut().is_ok());
                trace.borrow_mut().push(if event.dropped {
                    "native-drop"
                } else {
                    "native-hover"
                });
                events.borrow_mut().push(event);
                true
            }
        }));
        let offer = Offer {
            id: 17,
            requests: Rc::default(),
        };
        let mime = if files {
            transfer::URI_LIST
        } else {
            "application/x-test-token"
        };
        let generation = host.drag.borrow_mut().begin(
            offer.clone(),
            target.clone(),
            Point::default(),
            vec![mime.into()],
            9,
        );
        let payload = MimePayload::new(
            [MimeRepresentation::new(
                mime,
                if files {
                    &b"file:///fixture/a\r\n"[..]
                } else {
                    &b"opaque-token"[..]
                },
            )],
            SelectionIntent::Copy,
        );
        let item = transfer::decode(payload.representations().to_vec()).unwrap();
        assert!(host.drag.borrow_mut().complete(generation, item));
        action(&host, |offer| offer.id == 17, WEnum::Value(DndAction::Copy));
        Self {
            host,
            target,
            offer,
            events,
            generation,
        }
    }

    fn source_action(&self, action_value: WEnum<DndAction>) {
        action(&self.host, |offer| offer.id == self.offer.id, action_value);
    }
    fn drop(&self) {
        self.host.drag.borrow_mut().drop_pending();
        dispatch(&self.host);
    }
}

#[gpui::test]
fn source_action_after_enter_refreshes_callback_before_motion_feedback(
    cx: &mut gpui::TestAppContext,
) {
    let f = Fixture::new(cx, true);
    assert_eq!(&*f.target.trace.borrow(), &["native-hover", "enter"]);
    // A different process changes the negotiated action, with no destination key injection.
    f.source_action(WEnum::Value(DndAction::Move));
    assert_eq!(
        f.events.borrow().len(),
        2,
        "Action must republish after Enter"
    );
    assert_eq!(f.events.borrow()[1].action, Some(NativeDragAction::Move));
    assert_eq!(f.events.borrow()[1].modifiers, Modifiers::default());
    f.host.modifiers.set(Modifiers {
        shift: true,
        ..Modifiers::default()
    });
    let position = Point::new(px(91.), px(42.));
    motion(&f.host, position);
    let events = f.events.borrow();
    assert_eq!(events.len(), 3);
    assert_eq!(events[2].position, position);
    assert!(events[2].modifiers.shift && !events[2].dropped);
    assert_eq!(events[2].action, Some(NativeDragAction::Move));
    assert_eq!(events[2].data, events[0].data);
    assert_eq!(events[2].offered_types, events[0].offered_types);
    assert_eq!(
        &*f.target.trace.borrow(),
        &[
            "native-hover",
            "enter",
            "native-hover",
            "pending",
            "native-hover",
            "pending"
        ]
    );
    drop(events);
    f.drop();
    f.drop();
    assert_eq!(&*f.offer.requests.borrow(), &["finish"]);
    assert_eq!(
        f.target
            .inputs
            .borrow()
            .iter()
            .filter(|e| matches!(e, FileDropEvent::Entered { .. }))
            .count(),
        1
    );
    assert_eq!(
        f.target
            .inputs
            .borrow()
            .iter()
            .filter(|e| matches!(e, FileDropEvent::Submit { .. }))
            .count(),
        1
    );
}

#[gpui::test]
fn motion_alone_republishes_arbitrary_mime_and_local_modifiers(cx: &mut gpui::TestAppContext) {
    let f = Fixture::new(cx, false);
    f.host.modifiers.set(Modifiers {
        control: true,
        ..Modifiers::default()
    });
    motion(&f.host, Point::new(px(12.), px(34.)));
    assert_eq!(
        f.events.borrow().len(),
        2,
        "Motion must refresh native metadata"
    );
    assert!(f.events.borrow()[1].modifiers.control);
    assert_eq!(
        &*f.events.borrow()[1]
            .data
            .get("application/x-test-token")
            .unwrap(),
        b"opaque-token"
    );
    f.drop();
    assert!(f.events.borrow().last().unwrap().dropped);
    assert!(matches!(
        &f.target.inputs.borrow()[..],
        [FileDropEvent::Pending { .. }]
    ));
    assert_eq!(&*f.offer.requests.borrow(), &["finish"]);
}

#[gpui::test]
fn stale_offer_and_unknown_action_do_not_submit_or_finish(cx: &mut gpui::TestAppContext) {
    let f = Fixture::new(cx, true);
    action(
        &f.host,
        |offer| offer.id == 99,
        WEnum::Value(DndAction::Move),
    );
    assert_eq!(f.events.borrow().len(), 1);
    assert_eq!(f.host.drag.borrow().action, Some(NativeDragAction::Copy));
    f.source_action(WEnum::Unknown(0x80));
    assert_eq!(f.events.borrow().last().unwrap().action, None);
    f.drop();
    assert!(matches!(
        f.target.inputs.borrow().last(),
        Some(FileDropEvent::Exited)
    ));
    assert!(!f.events.borrow().iter().any(|e| e.dropped));
    assert_eq!(&*f.offer.requests.borrow(), &["refuse"]);
}

#[gpui::test]
fn refresh_callback_can_replace_generation_without_stale_pending_or_submit(
    cx: &mut gpui::TestAppContext,
) {
    for dropped in [false, true] {
        let f = Fixture::new(cx, true);
        let host = f.host.clone();
        let offer = Offer {
            id: 29,
            requests: Rc::default(),
        };
        let target = f.target.clone();
        f.host.common.borrow_mut().callbacks.native_drop = Some(Box::new(move |_| {
            host.drag.borrow_mut().begin(
                offer.clone(),
                target.clone(),
                Point::default(),
                vec![],
                10,
            );
            assert!(host.common.try_borrow_mut().is_ok());
            true
        }));
        if dropped {
            f.drop();
        } else {
            motion(&f.host, Point::new(px(1.), px(2.)));
        }
        assert_ne!(f.host.drag.borrow().generation, f.generation);
        assert_eq!(f.host.drag.borrow().offer.as_ref().unwrap().id, 29);
        assert_eq!(&*f.target.trace.borrow(), &["native-hover", "enter"]);
        assert!(f.offer.requests.borrow().is_empty());
    }
}

#[gpui::test]
fn delayed_payload_uses_latest_motion_and_drop_before_completion(cx: &mut gpui::TestAppContext) {
    let f = Fixture::new(cx, true);
    let generation = f.host.drag.borrow_mut().begin(
        f.offer.clone(),
        f.target.clone(),
        Point::default(),
        vec![transfer::URI_LIST.into()],
        10,
    );
    f.events.borrow_mut().clear();
    f.target.trace.borrow_mut().clear();
    motion(&f.host, Point::new(px(55.), px(66.)));
    f.source_action(WEnum::Value(DndAction::Move));
    f.host.drag.borrow_mut().drop_pending();
    dispatch(&f.host);
    assert!(f.events.borrow().is_empty());
    let payload = transfer::file_payload(["/fixture/b".into()], SelectionIntent::Copy).unwrap();
    assert!(
        !f.host
            .drag
            .borrow_mut()
            .complete(f.generation, ClipboardItem::new_mime(payload.clone()))
    );
    let item = transfer::decode(payload.representations().to_vec()).unwrap();
    assert!(f.host.drag.borrow_mut().complete(generation, item));
    dispatch(&f.host);
    assert_eq!(
        &*f.target.trace.borrow(),
        &["native-hover", "enter", "native-drop", "submit"]
    );
    assert!(
        f.events
            .borrow()
            .iter()
            .all(|e| e.position == Point::new(px(55.), px(66.)))
    );
}

#[gpui::test]
fn paths_without_native_callback_keep_legacy_lifecycle(cx: &mut gpui::TestAppContext) {
    let f = Fixture::new(cx, true);
    f.host.common.borrow_mut().callbacks.native_drop = None;
    motion(&f.host, Point::new(px(7.), px(8.)));
    f.drop();
    assert_eq!(
        &*f.target.trace.borrow(),
        &["native-hover", "enter", "pending", "submit"]
    );
    assert_eq!(&*f.offer.requests.borrow(), &["finish"]);
}

#[gpui::test]
fn refresh_does_not_restart_admission_and_leave_stops_feedback(cx: &mut gpui::TestAppContext) {
    let f = Fixture::new(cx, true);
    let calls = Rc::new(Cell::new(0));
    f.host.common.borrow_mut().callbacks.native_drop = Some(Box::new({
        let calls = calls.clone();
        move |_| {
            calls.set(calls.get() + 1);
            false
        }
    }));
    f.source_action(WEnum::Value(DndAction::Move));
    assert_eq!(calls.get(), 1);
    assert!(f.host.drag.borrow().entered && !f.host.drag.borrow().refused);
    assert!(f.offer.requests.borrow().is_empty());
    assert!(
        matches!(f.target.inputs.borrow().last(), Some(FileDropEvent::Pending { position }) if *position == Point::default())
    );
    // The protocol Leave owns Exited; subsequent Motion/Action must publish nothing.
    assert!(f.host.drag.borrow_mut().leave());
    Host::input(&f.target, FileDropEvent::Exited);
    motion(&f.host, Point::new(px(8.), px(9.)));
    f.source_action(WEnum::Value(DndAction::Copy));
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        f.target.inputs.borrow().last(),
        Some(FileDropEvent::Exited)
    ));
    assert_eq!(f.target.inputs.borrow().len(), 3);
}

#[gpui::test]
fn refresh_callback_replacement_is_retained_for_final_drop(cx: &mut gpui::TestAppContext) {
    let f = Fixture::new(cx, true);
    let replaced = Rc::new(Cell::new(0));
    f.host.common.borrow_mut().callbacks.native_drop = Some(Box::new({
        let host = f.host.clone();
        let replaced = replaced.clone();
        move |event| {
            assert!(!event.dropped);
            let replaced = replaced.clone();
            host.common.borrow_mut().callbacks.native_drop = Some(Box::new(move |event| {
                assert!(event.dropped);
                replaced.set(replaced.get() + 1);
                true
            }));
            true
        }
    }));
    motion(&f.host, Point::new(px(8.), px(9.)));
    f.drop();
    assert_eq!(replaced.get(), 1);
    assert_eq!(&*f.offer.requests.borrow(), &["finish"]);
}

#[gpui::test]
fn reentrant_enter_cannot_submit_a_superseded_generation(cx: &mut gpui::TestAppContext) {
    let f = Fixture::new(cx, true);
    f.host.drag.borrow_mut().entered = false;
    f.host.drag.borrow_mut().drop_pending();
    let host = f.host.clone();
    *f.target.on_input.borrow_mut() = Some(Box::new(move || host.drag.borrow_mut().clear()));
    dispatch(&f.host);
    assert_eq!(f.events.borrow().len(), 2);
    assert!(!f.events.borrow().iter().any(|e| e.dropped));
    assert!(
        !f.target
            .inputs
            .borrow()
            .iter()
            .any(|e| matches!(e, FileDropEvent::Submit { .. }))
    );
    assert!(f.offer.requests.borrow().is_empty());
}
