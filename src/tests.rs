use crate::{param::SystemParam, *};

#[test]
fn entities_despawn() {
    let mut entities = crate::entity::Entities::default();
    let a = entities.spawn();
    entities.despawn(a, &mut vec![]);
    assert!(!entities.is_alive(a));

    let mut alive_entities = Vec::new();
    let mut dead_entities = Vec::new();

    for _ in 0..10000 {
        alive_entities.push(entities.spawn());
    }

    for i in 0..5 {
        for j in 0..100 {
            let index = i * 1000 + j;
            let entity = alive_entities.remove(index);
            entities.despawn(entity, &mut Vec::new());
            dead_entities.push(entity);
        }
    }

    for entity in alive_entities.iter().copied() {
        assert!(entities.is_alive(entity));
    }
    for entity in dead_entities.iter().copied() {
        assert!(!entities.is_alive(entity));
    }
    dead_entities.clear();
    for _ in 0..500 {
        alive_entities.push(entities.spawn());
    }
    for entity in alive_entities.iter().copied() {
        assert!(entities.is_alive(entity));
    }
    while let Some(entity) = alive_entities.pop() {
        dead_entities.push(entity);
        entities.despawn(entity, &mut Vec::new());
    }
    for entity in dead_entities.iter().copied() {
        assert!(!entities.is_alive(entity));
    }
}


#[test]
fn link_resources() {
    struct A;
    struct B; impl Resource for B {}
    struct C; impl Resource for C {}

    impl Resource for A {
        fn join_additional_resource_access<F: FnMut(ResourceId)>(world: &mut World, mut f: F) -> Result<(), param::SystemParamError> {
            f(crate::param::get_resource_id::<B>(world)?);
            f(crate::param::get_resource_id::<C>(world)?);
            Ok(())
        }   
    }

    let mut access1 = crate::access::Access::default();
    let mut access2 = crate::access::Access::default();
    let mut world = World::new(1).unwrap();
    world.insert_resource(A);
    world.insert_resource(B);
    world.insert_resource(C);
    crate::Res::<A>::join_resource_access(&mut world, &mut access1).unwrap();
    crate::ResMut::<A>::join_resource_access(&mut world, &mut access2).unwrap();
    assert_eq!(access1.immutable().count_ones(), 3);
    assert_eq!(access2.mutable().count_ones(), 3);

    fn f1(_: Res<A>) {}
    fn f2(_: ResMut<A>) {}

    fn into_system<ParamIn: SystemInput, Output: SystemOutput, S: IntoSystem<ParamIn, (), Output> + 'static>(s: S) -> <S as IntoSystem<ParamIn, (), Output>>::System {
        s.into_system()
    }

    let mut system1 = into_system(f1);
    system1.init(&mut world).unwrap();
    assert_eq!((*system1.resource_access().immutable() & *access1.immutable()).count_ones(), 3);

    let mut system2 = into_system(f2);
    system2.init(&mut world).unwrap();
    assert_eq!((*system2.resource_access().mutable() & *access2.mutable()).count_ones(), 3);
}
