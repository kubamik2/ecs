mod common;
use ecs::*;

#[test]
fn layered() {
    let mut world = World::default();
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct A;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct B;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct C;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct D;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct E;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct F;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct G;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct H;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct I;
    #[derive(ScheduleLabel, PartialEq, Eq, Hash, Debug)] struct J;

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

    world.add_observer(|_: Trigger<A>, mut commands: Commands| {commands.insert_resource(SuccessA); commands.trigger(B, None);});
    world.add_observer(|_: Trigger<B>, mut commands: Commands| {commands.insert_resource(SuccessB); commands.trigger(C, None);});
    world.add_observer(|_: Trigger<C>, mut commands: Commands| {commands.insert_resource(SuccessC); commands.trigger(D, None);});
    world.add_observer(|_: Trigger<D>, mut commands: Commands| {commands.insert_resource(SuccessD); commands.trigger(E, None);});
    world.add_observer(|_: Trigger<E>, mut commands: Commands| {commands.insert_resource(SuccessE); commands.trigger(F, None);});
    world.add_observer(|_: Trigger<F>, mut commands: Commands| {commands.insert_resource(SuccessF); commands.trigger(G, None);});
    world.add_observer(|_: Trigger<G>, mut commands: Commands| {commands.insert_resource(SuccessG); commands.trigger(H, None);});
    world.add_observer(|_: Trigger<H>, mut commands: Commands| {commands.insert_resource(SuccessH); commands.trigger(I, None);});
    world.add_observer(|_: Trigger<I>, mut commands: Commands| {commands.insert_resource(SuccessI); commands.trigger(J, None);});
    world.add_observer(|_: Trigger<J>, mut commands: Commands| {commands.insert_resource(SuccessJ);});

    world.trigger(A, None).unwrap();

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
