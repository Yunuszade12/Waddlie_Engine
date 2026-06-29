use bevy::prelude::*;

fn main() {
    let mut app = App::new();

    // Boot up the system as a specialized Editor instead of the Web Player
    waddlie_core::boot_editor_base(&mut app);

    app.run();
}
