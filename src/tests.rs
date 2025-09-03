use crate::*;

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
    schedule.add_system(|| { println!("I am a system!"); }).unwrap();
    world.insert_schedule(Tick, schedule);
    world.run_schedule(&Tick);
    world.remove_schedule(&Tick).unwrap();
}

#[test]
fn remove_system() {
    let mut world = World::default();
    let mut schedule = Schedule::default();
    let id = schedule.add_system(|| { println!("I am a system!"); }).unwrap();
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

    #[derive(Event)]
    struct Tick;

    let mut world = World::default();
    world.insert_resource(Count(0));
    for _ in 0..2 {
        world.add_observer(|_: Signal<Tick>, mut count: ResMut<Count>| {
            count.0 += 1;
        }).unwrap();
    }

    for _ in 0..100 {
        world.send_signal(Tick, None);
    }

    assert!(world.resource::<Count>().0 == 200);
}
