policy HospitalPrivate {
    owner: "hospital-a";
    export: deny;
}

fn main() {
    let secret = private<HospitalPrivate>(85);

    // Rejected: the decision to print reveals a fact about `secret`.
    if secret > 80 {
        println("The protected score is greater than eighty");
    }
}
