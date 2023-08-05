mod screen;
mod draw_term;
mod constants;

fn main() {
    let mut draw_term = draw_term::DrawTerm::new();
    draw_term.run();
}
