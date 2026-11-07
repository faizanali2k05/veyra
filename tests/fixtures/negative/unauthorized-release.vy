policy HospitalPrivate {
    owner: "hospital-a";
    export: deny;
}

fn main() {
    let record = private<HospitalPrivate>("patient record");
    release(record);
}
