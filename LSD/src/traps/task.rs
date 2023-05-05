pub struct TaskData {
    trap_frame: super::TrapFrame,
    task_id: u128,
}

pub enum Privilege {
    Root,
    SuperUser,
    User,
    Guest
}