module Types exposing (..)


type alias Device =
    { id : String
    , kind : String
    , state : String
    , location : Int
    , manufacturer : Maybe String
    , provisionedAt : String
    }


type alias Dispatcher =
    { id : String
    , state : String
    , provisionedAt : String
    }
