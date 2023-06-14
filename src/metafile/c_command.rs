use std::path::Path;

/// returns true if `b` is a descendant of `a`'s parent
/// ## Note:
/// This is slightly broader than the linguistic definition of C-command when `a` is a dir, as it includes all nodes dominated by `a`.
pub fn c_commands(a: impl AsRef<Path>, b: impl AsRef<Path>) -> bool {
    let dir = a.as_ref().parent();
    b.as_ref()
        .ancestors()
        .position(|ancestor| Some(ancestor) == dir)
        .is_some()
}

#[cfg(test)]
mod test {
    use super::c_commands;
    /*
        Consider the directory structure:
        ```
        /
            animal/
                nya
                mew
                dog/
                    awrf
            meow
        ```
        Assume `nya`, `mew` are duplicates, and `meow`, `awrf` are duplicates
        We want to mark them up as follows:
        ```
        /
            animal/
                nya (Dup Inside, Unique Outside)
                mew (Dup Inside, Unique Outside)
                dog/
                    awrf (Dup Outside, Unique Inside)
            meow (Dup Inside, Unique Outside)
        ```
    */

    #[test]
    fn doesnt() {
        assert!(!c_commands("/animal/nya", "/meow"));
        assert!(!c_commands("/animal/dog/awrf", "/animal/nya"));
    }

    #[test]
    fn does() {
        assert!(c_commands("/animal/nya", "/animal/mew"));
        assert!(c_commands("/animal/nya", "/animal/dog/awrf"));
        assert!(c_commands("/meow", "/animal/dog/awrf"));
    }
}
