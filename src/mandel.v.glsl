#version 150
uniform vec2 world_bottom_left;
uniform vec2 world_dimensions;

in vec2 position;
in vec3 color;
out vec3 frag_color;

void main() {
	frag_color = color;

	vec2 world_pos = position;
	vec2 screen_pos = ((world_pos - world_bottom_left) / world_dimensions) * 2 - 1;

	gl_Position = vec4(screen_pos, 0.0, 1.0);
}

