policy HospitalPrivate {
    owner: "hospital-a";
    export: deny;
}

fn main() {
    let secret = private<HospitalPrivate>(100);
    let calculated = secret + 20;

    // Protected computation is allowed while it remains inside the policy boundary.
    let still_private = calculated * 2;
}
