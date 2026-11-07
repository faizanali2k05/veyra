policy ResearchPrivate {
    owner: "research-lab";
    export: deny;
}

fn normalize(value: int) -> int {
    return value * 2 + 1;
}

fn main() {
    let sample = private<ResearchPrivate>(20);
    let derived = normalize(sample);
    let retained = [derived, derived + 1];
}
