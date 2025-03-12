pub trait TakeFlag {
    fn take(&mut self, flag: Self) -> bool;
}

impl<T: bitflags::Flags + Copy> TakeFlag for T {
    fn take(&mut self, flag: Self) -> bool {
        let found = self.contains(flag);
        if found {
            self.remove(flag);
        }
        found
    }
}
