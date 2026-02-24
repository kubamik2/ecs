mod common;
use ecs::*;

#[test]
fn integration() {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct EventA(u32);
    static mut CNT: usize = 0;

    let mut world = World::default();
    world.register_event::<EventA>();

    let mut event_state = EventReaderState::<EventA>::new();
    assert!(event_state.reader(&world).unwrap().read().next().is_none());
    world.resource_mut::<EventQueue<EventA>>().send(EventA(12));
    world.resource_mut::<EventQueue<EventA>>().send(EventA(1));
    assert!(event_state.reader(&world).unwrap().read().next().copied() == Some(EventA(12)));

    let mut schedule = Schedule::default();
    schedule.add_system(|mut reader: EventReader<EventA>| {
        let next = reader.read().next().copied();
        if unsafe { CNT == 0 } {
            assert!(next == Some(EventA(12)));
        } else {
            assert!(next == Some(EventA(1)));
        }
        unsafe { CNT += 1 };
    });
    schedule.run(&mut world);

    world.resource_mut::<EventQueue<EventA>>().update();
    world.resource_mut::<EventQueue<EventA>>().update();

    let mut schedule = Schedule::default();
    schedule.add_system(|mut reader: EventReader<EventA>| {
        assert!(reader.read().next().is_none());
    });
    schedule.run(&mut world);
}
