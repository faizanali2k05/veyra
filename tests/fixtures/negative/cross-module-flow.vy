import privacy_module;

policy HospitalPrivate {
    owner: "hospital-a";
    export: deny;
}

fn main() {
    let record = private<HospitalPrivate>("patient record");
    privacy_module.expose(record);
}
