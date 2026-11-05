//! bool scratch (not landed): CASE=<dir>/<name> OBJ=<index> TOOLS=<i,j,..> OUT=<dir>: a smaller case of a recorded one's solids, recorded with OCCT's single Boolean.
use noshape_kernel::*;
fn main() {
    let _ = set_working_scale(1000.0);
    let case = std::env::var("CASE").unwrap();
    let (dir, name) = case.rsplit_once('/').unwrap();
    let c = bool_corpus::load(std::path::Path::new(dir), name).unwrap();
    let all: Vec<Shape> = c.objects.iter().chain(c.tools.iter()).cloned().collect();
    let obj = vec![all[std::env::var("OBJ").unwrap().parse::<usize>().unwrap()].clone()];
    let tools: Vec<Shape> = std::env::var("TOOLS").unwrap().split(',').map(|i| all[i.parse::<usize>().unwrap()].clone()).collect();
    unsafe { std::env::set_var("NOSHAPE_BOOL_NO_TREE", "1") };
    set_rust_boolean(Some(false));
    let out = std::path::PathBuf::from(std::env::var("OUT").unwrap());
    let op = match std::env::var("OP").as_deref() { Ok("cut") => BooleanOp::Cut, Ok("common") => BooleanOp::Common, Ok("fuse") => BooleanOp::Fuse, _ => c.op };
    let opts = if std::env::var_os("NOGLUE").is_some() { BooleanOptions { glue: Glue::Off, ..c.opts } } else { c.opts };
    let r = bool_corpus::record(&out, op, &obj, &tools, &opts, || boolean(op, &obj, &tools, &opts)).unwrap();
    println!("faces {} volume {:e} path {}", r.shape.count(ShapeType::Face), r.shape.volume().unwrap(), last_boolean_path());
    for f in std::fs::read_dir(&out).unwrap() { println!("{:?}", f.unwrap().path()); }
}
