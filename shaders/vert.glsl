#version 450
layout(location = 0) in vec2 inPosition;
layout(push_constant) uniform PushConstants {
    mat4 mvp;
} pc;

void main() {
    gl_Position = pc.mvp * vec4(inPosition, 0.0, 1.0);
}