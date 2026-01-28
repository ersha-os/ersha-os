module SvgAssets exposing (logo)

import Html exposing (Html)
import Html.Attributes exposing (attribute)
import Svg exposing (path, svg)
import Svg.Attributes exposing (class, d, fill, viewBox)


logo : Html msg
logo =
    svg [ class "w-6 h-6 text-white", fill "none", attribute "stroke" "currentColor", viewBox "0 0 24 24" ]
        [ path [ d "M13 10V3L4 14h7v7l9-11h-7z", attribute "stroke-linecap" "round", attribute "stroke-linejoin" "round", attribute "stroke-width" "2" ]
            []
        ]
