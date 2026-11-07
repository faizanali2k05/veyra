struct Student {
    name: string,
    age: int,
}

fn factorial(n: int) -> int {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}

fn main() {
    let student = Student {
        name: "Ali",
        age: 22,
    };
    let scores = [90, 85, 95];
    println(student.name);
    println(scores[0]);
    println(factorial(5));
    for i in 0..3 {
        println(i);
    }
}
