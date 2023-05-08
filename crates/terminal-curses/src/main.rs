use cursive::views;
use cursive::{Cursive, CursiveExt};

pub fn main() {
    let mut siv = Cursive::new();
    siv.add_layer(views::ScrollView::new(views::DebugView::new()));
    siv.run();
}
