const STACK_LEN: u32 = 24u;
struct Stack {
    arr: array<u32, STACK_LEN>,
	head: u32,
}

fn stack_new() -> Stack {
    var arr: array<u32, STACK_LEN>;
    return Stack(arr, 0u);
}

fn stack_push(stack: ptr<function, Stack>, val: u32) {
    (*stack).arr[(*stack).head] = val;
    (*stack).head += 1u;
}

fn stack_pop(stack: ptr<function, Stack>) -> u32 {
    (*stack).head -= 1u;
    return (*stack).arr[(*stack).head];
}
