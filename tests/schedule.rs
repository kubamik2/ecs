mod common;
use ecs::*;

#[test]
fn insert() {
    #[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug)]
    struct Tick;

    let mut world = World::default();
    let mut schedule = Schedule::default();
    schedule.add_system(|| { println!("I am a system!"); });
    world.insert_schedule(Tick, schedule);
    world.run_schedule(Tick);
    world.remove_schedule(&Tick).unwrap();
}

#[test]
#[should_panic]
fn disallow_multiple_worlds() {
    let mut world1 = World::new(1).unwrap();
    let mut world2 = World::new(1).unwrap();

    let mut schedule = Schedule::default();
    schedule.run(&mut world1);
    schedule.run(&mut world2);
} 
