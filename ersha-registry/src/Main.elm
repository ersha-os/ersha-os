module Main exposing (main, subscriptions)

import Browser exposing (Document)
import Html exposing (..)
import Html.Attributes exposing (class)


type alias Model =
    {}


type Msg
    = NoOp


view : Model -> Document Msg
view _ =
    { title = "ersha-registry"
    , body =
        [ h1 [ class "text-3xl underline" ] [ text "Hello Ersha World" ]
        ]
    }


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        NoOp ->
            ( model, Cmd.none )


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.none


init : () -> ( Model, Cmd msg )
init _ =
    ( Model, Cmd.none )


main : Program () Model Msg
main =
    Browser.document
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }
