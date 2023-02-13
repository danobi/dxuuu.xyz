// Rounder curves
$fn = 64;

// Radius available to the spice bottle. ie. the "holdable circle".
//
// The base model's effective radius was ~22.6mm. McCormick bottles
// have a head diameter of ~46mm and neck diameter of ~35mm. So the
// effective radius will need to be between ~17.5mm and ~23mm
inner_radius = 20;
// Radius of outer circle
outer_radius = inner_radius + 3;
// How much the inner and outer radii will get added on
minkowski_radius = 1;

// Create the backbone
translate([(225/2), 25, 0])
    cube([225, 5, 15], center=true);

// Create 4 arms
for (i=[0:3]) {
    // Move this copy along the X axis appropriate amount
    translate([i*56.5 + 28, 2, 0]) {
        linear_extrude(15, center=true) {
            // Round off the tips of the arms
            minkowski() {
                // Hollow out "cheese wheel"
                difference() {
                    // Create sliced off "cheese wheel"
                    intersection() {
                        // Create 2d circle
                        circle(outer_radius-minkowski_radius);
                        // Create rectangle to take slice of cylinder
                        translate([0, 7.8, 0])   
                            square([500, 41-minkowski_radius], center=true);
                    }
                    circle(inner_radius+minkowski_radius);
                }
                circle(minkowski_radius);
            }
        }
    }
}
