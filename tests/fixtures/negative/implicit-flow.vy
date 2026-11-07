policy HospitalPrivate {
    owner: "hospital-a";
    export: deny;
}

fn main() {
    let secret = private<HospitalPrivate>(85);
    if secret > 80 {
        println("high score");
    }
}
