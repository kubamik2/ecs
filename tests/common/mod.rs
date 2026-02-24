use ecs::*;

#[derive(Component)]
pub struct ComponentA {
    pub a: u128,
    pub b: u64,
    pub c: u32,
    pub d: u16,
    pub e: u8,
}

impl ComponentA {
    pub fn new(i: usize) -> Self {
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
    pub fn validate(&self, i: usize) -> bool {
        let a = self.a;
        let b = self.b as u128;
        let c = self.c as u128;
        let d = self.d as u128;
        let e = self.e as u128;
        i as u128 == a && a == b && b == c && c == d && d == e
    }
}

#[derive(Component)]
pub struct ComponentB(pub String);
impl ComponentB {
    pub fn validate(&self, i: usize) -> bool {
        format!("{i}") == self.0
    }
}


pub fn panic_unit() {
    panic!();
}
