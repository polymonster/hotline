struct vs_input {
    float4 position : POSITION;
    float4 colour: COLOR;
};

struct vs_output {
    float4 position : SV_POSITION0;
    float4 colour: COLOR;
};

struct ps_output {
    float4 colour : SV_Target;
};

vs_output vs_main( vs_input input ) {
    vs_output output;
    output.position = input.position;
    output.colour = input.colour;
    return output;
}

ps_output ps_main( vs_output input ) {
    ps_output output;
    output.colour = input.colour;
    return output;
}