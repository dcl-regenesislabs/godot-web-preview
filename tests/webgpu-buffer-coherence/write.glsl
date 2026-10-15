#[compute]
#version 450
layout(local_size_x = 1) in;
layout(set = 0, binding = 0, std430) buffer Input { uint values[]; } input_data;
layout(set = 0, binding = 1, std430) buffer Output { uint value; } output_data;
void main() {
    input_data.values[0] = 42;
    output_data.value = input_data.values[0];
}
