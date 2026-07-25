#define_import_path bevy_pointcloud::simple_material_functions

#import bevy_pointcloud::simple_material_types::ColorStop

// Function to evaluate the multi-stop gradient based on shaderdefs
fn evaluate_gradient(base_color: vec4<f32>, end_color: vec4<f32>, color_stops: array<ColorStop, 8>, t: f32) -> vec4<f32> {
    let factor = clamp(t, 0.0, 1.0);

    #ifdef SIMPLE_MATERIAL_GRADIENT
        var current_start_color = base_color;
        var current_start_point = 0.0;

        // Cumulative chain for 8 color stops
        #ifdef SIMPLE_MATERIAL_COLOR_STOP_1
            if factor < color_stops[0].point {
                let local_t = (factor - current_start_point) / (color_stops[0].point - current_start_point);
                return mix(current_start_color, color_stops[0].color, local_t);
            }
            current_start_color = color_stops[0].color;
            current_start_point = color_stops[0].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_2
            if factor < color_stops[1].point {
                let local_t = (factor - current_start_point) / (color_stops[1].point - current_start_point);
                return mix(current_start_color, color_stops[1].color, local_t);
            }
            current_start_color = color_stops[1].color;
            current_start_point = color_stops[1].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_3
            if factor < color_stops[2].point {
                let local_t = (factor - current_start_point) / (color_stops[2].point - current_start_point);
                return mix(current_start_color, color_stops[2].color, local_t);
            }
            current_start_color = color_stops[2].color;
            current_start_point = color_stops[2].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_4
            if factor < color_stops[3].point {
                let local_t = (factor - current_start_point) / (color_stops[3].point - current_start_point);
                return mix(current_start_color, color_stops[3].color, local_t);
            }
            current_start_color = color_stops[3].color;
            current_start_point = color_stops[3].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_5
            if factor < color_stops[4].point {
                let local_t = (factor - current_start_point) / (color_stops[4].point - current_start_point);
                return mix(current_start_color, color_stops[4].color, local_t);
            }
            current_start_color = color_stops[4].color;
            current_start_point = color_stops[4].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_6
            if factor < color_stops[5].point {
                let local_t = (factor - current_start_point) / (color_stops[5].point - current_start_point);
                return mix(current_start_color, color_stops[5].color, local_t);
            }
            current_start_color = color_stops[5].color;
            current_start_point = color_stops[5].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_7
            if factor < color_stops[6].point {
                let local_t = (factor - current_start_point) / (color_stops[6].point - current_start_point);
                return mix(current_start_color, color_stops[6].color, local_t);
            }
            current_start_color = color_stops[6].color;
            current_start_point = color_stops[6].point;
        #endif

        #ifdef SIMPLE_MATERIAL_COLOR_STOP_8
            if factor < color_stops[7].point {
                let local_t = (factor - current_start_point) / (color_stops[7].point - current_start_point);
                return mix(current_start_color, color_stops[7].color, local_t);
            }
            current_start_color = color_stops[7].color;
            current_start_point = color_stops[7].point;
        #endif

        // Final interpolation between the last active stop and end_color
        let final_t = (factor - current_start_point) / (1.0 - current_start_point);
        return mix(current_start_color, end_color, final_t);

    #else
        return base_color;
    #endif
}
