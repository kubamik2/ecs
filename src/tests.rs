use crate::{resource::Changed, world::WorldPtr, *};

#[derive(Component)]
struct ComponentA {
    a: u128,
    b: u64,
    c: u32,
    d: u16,
    e: u8,
}

impl ComponentA {
    fn new(i: usize) -> Self {
        Self {
            a: i as u128,
            b: i as u64,
            c: i as u32,
            d: i as u16,
            e: i as u8,
        }
    }
}

impl ComponentA {
    fn validate(&self, i: usize) -> bool {
        let a = self.a;
        let b = self.b as u128;
        let c = self.c as u128;
        let d = self.d as u128;
        let e = self.e as u128;
        i as u128 == a && a == b && b == c && c == d && d == e
    }
}

#[derive(Component)]
struct ComponentB(String);
impl ComponentB {
    fn validate(&self, i: usize) -> bool {
        format!("{i}") == self.0
    }
}

#[test]
fn get_component() {
    let mut world = World::default();
    let mut entities = vec![];
    for i in 0..100 {
        let entity = world.spawn((
            ComponentA::new(i),
            ComponentB(format!("{i}")),
        ));
        entities.push(entity);
    }
    (0..100).for_each(|i| {
        let component = world.get_component::<ComponentA>(entities[i]).expect("get_component ComponentA not found");
        assert!(component.validate(i), "ComponentA validation failed");
    });
    (0..100).for_each(|i| {
        let component = world.get_component::<ComponentB>(entities[i]).expect("get_component ComponentB not found");
        assert!(component.validate(i), "ComponentB validation failed");
    });
}

#[test]
fn set_component() {
    let mut world = World::default();
    let mut entities = vec![];
    for i in 0..100 {
        let entity = world.spawn((
            ComponentA::new(i),
            ComponentB(format!("{i}")),
        ));
        entities.push(entity);
    }
    (0..100).for_each(|i| {
        world.set_component(entities[i], ComponentA::new(i+1));
        world.set_component(entities[i], ComponentB(format!("{}", i+1)));
    });
    (0..100).for_each(|i| {
        let component = world.get_component::<ComponentA>(entities[i]).unwrap();
        assert!(component.validate(i+1), "ComponentA validation failed");
    });
    (0..100).for_each(|i| {
        let component = world.get_component::<ComponentB>(entities[i]).unwrap();
        assert!(component.validate(i+1), "ComponentB validation failed");
    });
}

#[test]
fn add_schedule() {
    #[derive(ScheduleLabel, Hash, PartialEq, Eq)]
    struct Tick;

    let mut world = World::default();
    let mut schedule = Schedule::default();
    schedule.add_system(|| { println!("I am a system!"); });
    world.insert_schedule(Tick, schedule);
    world.run_schedule(Tick);
    world.remove_schedule(&Tick).unwrap();
}

#[test]
fn remove_system() {
    let mut world = World::default();
    let mut schedule = Schedule::default();
    let id = schedule.add_system(|| { println!("I am a system!"); });
    world.remove_system(id);
    schedule.run(&mut world);
    assert!(schedule.is_empty());
}

#[test]
fn entities_despawn() {
    let mut entities = crate::entity::Entities::new();
    let a = entities.spawn();
    entities.despawn(a);
    assert!(!entities.is_alive(a));
}

#[test]
fn simple_observer() {
    #[derive(Resource)]
    struct Count(u32);

    struct Tick;

    let mut world = World::default();
    world.insert_resource(Count(0));
    for _ in 0..2 {
        world.add_observer(|_: Signal<Tick>, mut count: ResMut<Count>| {
            count.0 += 1;
        });
    }

    for _ in 0..100 {
        world.send_signal(Tick, None);
    }

    assert!(world.resource::<Count>().0 == 200);
}

#[test]
fn events() {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct EventA(u32);
    static mut CNT: usize = 0;

    let mut world = World::default();
    world.register_event::<EventA>();

    let mut event_state = EventReaderState::<EventA>::new();
    assert!(event_state.reader(&world).read().next().is_none());
    world.resource_mut::<EventQueue<EventA>>().send(EventA(12));
    world.resource_mut::<EventQueue<EventA>>().send(EventA(1));
    assert!(event_state.reader(&world).read().next().copied() == Some(EventA(12)));

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

#[test]
fn change_detection() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static X: AtomicU32 = AtomicU32::new(0);
    #[derive(Resource)]
    struct Count(u32);
    let mut world = World::default();
    world.insert_resource(Count(0));
    world.add_observer(|_: Signal<Changed<Count>>| {
        X.fetch_add(1, Ordering::Relaxed);
    });
    let mut schedule = Schedule::default();
    schedule.add_system(|mut count: ResMut<Count>| {
        count.0 += 1;
    });

    let mut count = world.get_resource_mut::<Count>().unwrap();
    count.0 += 1;
    drop(count);

    schedule.run(&mut world);
    assert!(X.load(Ordering::Relaxed) == 2, "{}", X.load(Ordering::Relaxed));
}

#[test]
fn query_filter() {
    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;
    #[derive(Component)]
    struct C;
    #[derive(Component)]
    struct D;
    #[derive(Component)]
    struct E;
    let mut world = World::default();
    let a = world.spawn((A, B, C, D));
    let b = world.spawn(A);
    let _ = world.spawn(B);
    let _ = world.spawn(C);
    let _ = world.spawn(D);

    assert!(world.query_filtered::<(), With<(A, B, C, D)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<(A, B)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<(C, D)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<(B, C)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<A>>().iter().count() == 2);
    assert!(world.query_filtered::<(), With<B>>().iter().count() == 2);
    assert!(world.query_filtered::<(), With<C>>().iter().count() == 2);
    assert!(world.query_filtered::<(), With<D>>().iter().count() == 2);
    assert!(world.query_filtered::<(), With<(A, A, B, B)>>().iter().count() == 1);
    assert!(world.query_filtered::<(), With<E>>().iter().count() == 0);
    assert!(world.query_filtered::<(), With<(A, E)>>().iter().count() == 0);

    assert!(world.query_filtered::<&A, With<A>>().get(a).is_some());
    assert!(world.query_filtered::<&A, Without<A>>().iter().count() == 0);
    assert!(world.query_filtered::<&A, Without<B>>().iter().count() == 1);
    assert!(world.query_filtered::<&A, Without<B>>().get(b).is_some());
    assert!(world.query_filtered::<(&A, &B, &C, &D), Without<A>>().iter().count() == 0);
}

#[test]
fn component_hooks() {
    struct Spawner;

    impl Component for Spawner {
        fn on_add(&mut self, commands: &mut Commands) {
            commands.spawn(Position);
        }

        fn on_remove(&mut self, commands: &mut Commands) {
            commands.spawn(Rotation);
        }
    }

    impl Resource for Spawner {
        fn on_add(&mut self, commands: &mut Commands) {
            commands.spawn(Position);
        }

        fn on_remove(&mut self, commands: &mut Commands) {
            commands.spawn(Rotation);
        }
    }

    #[derive(Component)]
    struct Position;
    #[derive(Component)]
    struct Rotation;

    let mut world = World::default();

    let entity1 = world.spawn(Spawner);
    let entity2 = world.spawn(Spawner);
    assert!(world.query::<&Position>().iter().count() == 2);
    world.despawn(entity1);
    world.despawn(entity2);
    assert!(world.query::<&Rotation>().iter().count() == 2);

    world.insert_resource(Spawner);
    world.insert_resource(Spawner);
    assert!(world.query::<&Position>().iter().count() == 4);
    world.remove_resource::<Spawner>();
    assert!(world.query::<&Rotation>().iter().count() == 4);
}

#[test]
fn recurent_commands() {
    let mut world = World::default();
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct A;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct B;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct C;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct D;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct E;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct F;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct G;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct H;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct I;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash)] struct J;

    #[derive(Resource)] pub struct SuccessA;
    #[derive(Resource)] pub struct SuccessB;
    #[derive(Resource)] pub struct SuccessC;
    #[derive(Resource)] pub struct SuccessD;
    #[derive(Resource)] pub struct SuccessE;
    #[derive(Resource)] pub struct SuccessF;
    #[derive(Resource)] pub struct SuccessG;
    #[derive(Resource)] pub struct SuccessH;
    #[derive(Resource)] pub struct SuccessI;
    #[derive(Resource)] pub struct SuccessJ;

    world.add_system(A, |mut commands: Commands| {commands.run_schedule(B); commands.insert_resource(SuccessA);});
    world.add_system(B, |mut commands: Commands| {commands.run_schedule(C); commands.insert_resource(SuccessB);});
    world.add_system(C, |mut commands: Commands| {commands.run_schedule(D); commands.insert_resource(SuccessC);});
    world.add_system(D, |mut commands: Commands| {commands.run_schedule(E); commands.insert_resource(SuccessD);});
    world.add_system(E, |mut commands: Commands| {commands.run_schedule(F); commands.insert_resource(SuccessE);});
    world.add_system(F, |mut commands: Commands| {commands.run_schedule(G); commands.insert_resource(SuccessF);});
    world.add_system(G, |mut commands: Commands| {commands.run_schedule(H); commands.insert_resource(SuccessG);});
    world.add_system(H, |mut commands: Commands| {commands.run_schedule(I); commands.insert_resource(SuccessH);});
    world.add_system(I, |mut commands: Commands| {commands.run_schedule(J); commands.insert_resource(SuccessI);});
    world.add_system(J, |mut commands: Commands| commands.insert_resource(SuccessJ));
    
    world.run_schedule(A);

    assert!(world.get_resource::<SuccessA>().is_some());
    assert!(world.get_resource::<SuccessB>().is_some());
    assert!(world.get_resource::<SuccessC>().is_some());
    assert!(world.get_resource::<SuccessD>().is_some());
    assert!(world.get_resource::<SuccessE>().is_some());
    assert!(world.get_resource::<SuccessF>().is_some());
    assert!(world.get_resource::<SuccessG>().is_some());
    assert!(world.get_resource::<SuccessH>().is_some());
    assert!(world.get_resource::<SuccessI>().is_some());
    assert!(world.get_resource::<SuccessJ>().is_some());

    let mut world = World::default();

    world.add_observer(|_: Signal<A>, mut commands: Commands| {commands.insert_resource(SuccessA); commands.send_signal(B, None);});
    world.add_observer(|_: Signal<B>, mut commands: Commands| {commands.insert_resource(SuccessB); commands.send_signal(C, None);});
    world.add_observer(|_: Signal<C>, mut commands: Commands| {commands.insert_resource(SuccessC); commands.send_signal(D, None);});
    world.add_observer(|_: Signal<D>, mut commands: Commands| {commands.insert_resource(SuccessD); commands.send_signal(E, None);});
    world.add_observer(|_: Signal<E>, mut commands: Commands| {commands.insert_resource(SuccessE); commands.send_signal(F, None);});
    world.add_observer(|_: Signal<F>, mut commands: Commands| {commands.insert_resource(SuccessF); commands.send_signal(G, None);});
    world.add_observer(|_: Signal<G>, mut commands: Commands| {commands.insert_resource(SuccessG); commands.send_signal(H, None);});
    world.add_observer(|_: Signal<H>, mut commands: Commands| {commands.insert_resource(SuccessH); commands.send_signal(I, None);});
    world.add_observer(|_: Signal<I>, mut commands: Commands| {commands.insert_resource(SuccessI); commands.send_signal(J, None);});
    world.add_observer(|_: Signal<J>, mut commands: Commands| {commands.insert_resource(SuccessJ);});

    world.send_signal(A, None);

    assert!(world.get_resource::<SuccessA>().is_some());
    assert!(world.get_resource::<SuccessB>().is_some());
    assert!(world.get_resource::<SuccessC>().is_some());
    assert!(world.get_resource::<SuccessD>().is_some());
    assert!(world.get_resource::<SuccessE>().is_some());
    assert!(world.get_resource::<SuccessF>().is_some());
    assert!(world.get_resource::<SuccessG>().is_some());
    assert!(world.get_resource::<SuccessH>().is_some());
    assert!(world.get_resource::<SuccessI>().is_some());
    assert!(world.get_resource::<SuccessJ>().is_some());
}
