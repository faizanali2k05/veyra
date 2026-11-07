policy HospitalPrivate {
    owner: "hospital-a";
    export: deny;
}

fn main() {
    let secret = private<HospitalPrivate>(100);

    // Direct explicit flow: rejected.
    println(secret);

    // Derived explicit flow: also rejected.
    let calculated = secret + 20;
    println(calculated);
}
