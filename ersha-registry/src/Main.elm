module Main exposing (main, subscriptions)

import Browser exposing (Document)
import Html exposing (..)
import Html.Attributes exposing (attribute, class, placeholder, type_, value)
import Html.Events exposing (onClick, onInput)
import Http
import Json.Decode as Decode exposing (Decoder, field, int, maybe, string)
import Json.Encode as Encode
import SvgAssets
import Types exposing (Device, Dispatcher)


type alias Model =
    { devices : ApiData (List Device)
    , dispatchers : ApiData (List Dispatcher)
    , modal : Modals
    }


type ApiData a
    = Loading
    | Failure Http.Error
    | Success a
    | NotAsked


type Modals
    = DispatcherModal DispatcherForm
    | DeviceModal DeviceForm
    | Closed


type alias H3Cell =
    Int


type alias Ulid =
    String


type alias DispatcherForm =
    { id : Maybe Ulid
    , location : H3Cell
    }


type alias DeviceForm =
    { id : Maybe Ulid
    , location : H3Cell
    , kind : Maybe DeviceKind
    , manufacturer : Maybe String
    , sensors : List SensorForm
    }


type DeviceKind
    = Sensor


type alias SensorForm =
    { id : Maybe Ulid
    , kind : SensorKind
    }


type SensorKind
    = SoilMoisture
    | SoilTemp
    | AirTemp
    | Humidity
    | RainFall


type Msg
    = NoOp
      -- Navigation/UI
    | OpenModal Modals
    | CloseModal
      -- Forms
    | UpdateDispatcherForm DispatcherForm
    | UpdateDeviceFrom DeviceForm
      -- Api Lifecycle
    | FetchAll
    | GotDispatchers (Result Http.Error (List Dispatcher))
    | GotDevices (Result Http.Error (List Device))
    | SubmittedDispatcher (Result Http.Error ())
    | SubmittedDevice (Result Http.Error ())
    | SubmitForm Modals


view : Model -> Document Msg
view model =
    { title = "ersha-registry"
    , body =
        [ div [ class "min-h-screen bg-[#0b0c0e] text-[#d8d9da] font-sans" ]
            [ navBar
            , mainContent model
            , case model.modal of
                DispatcherModal form ->
                    viewDispatcherModal form

                DeviceModal _ ->
                    text "pending..."

                Closed ->
                    text ""
            ]
        ]
    }


viewDispatcherModal : DispatcherForm -> Html Msg
viewDispatcherModal form =
    div [ class "fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm" ]
        [ -- Modal Container
          div [ class "bg-[#181b1f] border border-[#2c2c2e] rounded-md shadow-2xl w-full max-w-md overflow-hidden" ]
            [ -- Header
              div [ class "bg-[#222529] px-6 py-4 border-b border-[#2c2c2e] flex justify-between items-center" ]
                [ h3 [ class "text-[#d8d9da] text-sm font-bold uppercase tracking-wider" ] [ text "Register Dispatcher" ]
                , button
                    [ class "text-gray-500 hover:text-white transition-colors"
                    , onClick CloseModal
                    ]
                    [ text "✕" ]
                ]

            -- Body / Form
            , div [ class "p-6 space-y-5" ]
                [ -- ID Field
                  div [ class "space-y-1.5" ]
                    [ label [ class "text-xs font-semibold text-orange-400 uppercase" ] [ text "Dispatcher ID (Optional)" ]
                    , input
                        [ type_ "text"
                        , placeholder "Leave blank to auto-generate"
                        , class "w-full bg-[#0b0c0e] border border-[#2c2c2e] text-[#d8d9da] rounded px-3 py-2 text-sm font-mono focus:border-orange-500 focus:outline-none transition-colors"
                        , value (Maybe.withDefault "" form.id)
                        , onInput
                            (\val ->
                                UpdateDispatcherForm
                                    { form
                                        | id =
                                            if val == "" then
                                                Nothing

                                            else
                                                Just val
                                    }
                            )
                        ]
                        []
                    ]

                -- Location Field
                , div [ class "space-y-1.5" ]
                    [ label [ class "text-xs font-semibold text-orange-400 uppercase" ] [ text "H3 Cell Index" ]
                    , input
                        [ type_ "number"
                        , class "w-full bg-[#0b0c0e] border border-[#2c2c2e] text-[#d8d9da] rounded px-3 py-2 text-sm font-mono focus:border-orange-500 focus:outline-none transition-colors"
                        , value (String.fromInt form.location)
                        , onInput (\val -> UpdateDispatcherForm { form | location = String.toInt val |> Maybe.withDefault 0 })
                        ]
                        []
                    , p [ class "text-[10px] text-gray-500 italic" ] [ text "Specify the hexagonal grid index for geographic dispatching." ]
                    ]
                ]

            -- Footer / Actions
            , div [ class "bg-[#222529] px-6 py-4 flex justify-end gap-3" ]
                [ button
                    [ class "px-4 py-2 text-sm font-medium text-[#d8d9da] hover:bg-[#2c2c2e] rounded transition-colors"
                    , onClick CloseModal
                    ]
                    [ text "Cancel" ]
                , button
                    [ class "px-4 py-2 text-sm font-medium text-white bg-orange-600 hover:bg-orange-500 rounded shadow-lg shadow-orange-900/20 transition-all active:scale-95"
                    , onClick (SubmitForm (DispatcherModal form))
                    ]
                    [ text "Register Dispatcher" ]
                ]
            ]
        ]


mainContent : Model -> Html Msg
mainContent model =
    main_ [ class "p-6 space-y-6" ]
        [ viewSummary
        , viewRemoteContent "Dispatchers" model.dispatchers viewDispatchers
        , viewRemoteContent "Devices" model.devices viewDevices
        ]


viewDevices : List Device -> Html Msg
viewDevices devices =
    div
        [ class "bg-[#181b1f] rounded border border-[#2c2c2e]" ]
        [ div
            [ class "px-4 py-3 border-b border-[#2c2c2e] flex justify-between items-center" ]
            [ h2
                [ class "text-sm font-bold uppercase text-gray-400" ]
                [ text "Registered Devices (/api/devices)" ]
            , span
                [ class "text-xs text-gray-500" ]
                [ text ("Total: " ++ String.fromInt (List.length devices)) ]
            ]
        , div
            [ class "overflow-x-auto" ]
            [ table
                [ class "w-full text-left text-sm" ]
                [ thead
                    [ class "bg-[#222529] text-gray-300 uppercase text-xs" ]
                    [ tr []
                        [ th [ class "px-4 py-2" ] [ text "ID" ]
                        , th [ class "px-4 py-2" ] [ text "Kind" ]
                        , th [ class "px-4 py-2" ] [ text "State" ]
                        , th [ class "px-4 py-2" ] [ text "Location" ]
                        , th [ class "px-4 py-2 text-right" ] [ text "Provisioned" ]
                        ]
                    ]
                , tbody
                    [ class "divide-y divide-[#2c2c2e]" ]
                    (List.map viewDeviceRow devices)
                ]
            ]
        , viewPagination
        ]


viewDeviceRow : Device -> Html Msg
viewDeviceRow device =
    tr
        [ class "hover:bg-[#222529] transition" ]
        [ td
            [ class "px-4 py-3 font-mono text-orange-400" ]
            [ text device.id ]
        , td
            [ class "px-4 py-3 text-gray-300" ]
            [ text device.kind ]
        , td
            [ class "px-4 py-3" ]
            [ span
                [ class (stateBadgeClass device.state) ]
                [ text device.state ]
            ]
        , td
            [ class "px-4 py-3 text-gray-400 font-mono" ]
            [ text (String.fromInt device.location) ]
        , td
            [ class "px-4 py-3 text-right text-gray-400 text-xs" ]
            [ text device.provisionedAt ]
        ]


stateBadgeClass : String -> String
stateBadgeClass state =
    let
        base =
            "px-2 py-0.5 rounded border text-xs font-medium "
    in
    case String.toLower state of
        "online" ->
            base ++ "bg-green-900/30 text-green-400 border-green-800"

        "offline" ->
            base ++ "bg-red-900/30 text-red-400 border-red-800"

        "idle" ->
            base ++ "bg-yellow-900/30 text-yellow-400 border-yellow-800"

        _ ->
            base ++ "bg-gray-800 text-gray-400 border-gray-700"


viewRemoteContent : String -> ApiData a -> (a -> Html Msg) -> Html Msg
viewRemoteContent label remoteData successView =
    case remoteData of
        NotAsked ->
            viewPlaceholder ("Initialize " ++ label ++ "...")

        Loading ->
            div [ class "flex flex-col items-center justify-center p-12 space-y-4" ]
                [ -- Grafana-style spinner
                  div [ class "w-8 h-8 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" ] []
                , p [ class "text-sm text-gray-500 font-mono animate-pulse" ] [ text ("Loading " ++ label ++ "...") ]
                ]

        Failure err ->
            div [ class "bg-red-900/20 border border-red-500/50 p-6 rounded-md" ]
                [ h3 [ class "text-red-400 text-sm font-bold uppercase mb-2" ] [ text (label ++ " Load Error") ]
                , p [ class "text-xs text-red-200/70 font-mono" ] [ text (httpErrorToString err) ]
                , button
                    [ class "mt-4 text-xs bg-red-500/20 hover:bg-red-500/40 text-red-200 px-3 py-1 rounded border border-red-500/50 transition"
                    , onClick FetchAll
                    ]
                    [ text "Retry Connection" ]
                ]

        Success data ->
            successView data



-- Helper for empty/initial states


viewPlaceholder : String -> Html Msg
viewPlaceholder msg =
    div [ class "p-12 border border-dashed border-[#2c2c2e] rounded text-center text-gray-600 italic text-sm" ]
        [ text msg ]


viewDispatchers : List Dispatcher -> Html Msg
viewDispatchers dispatchers =
    div
        [ class "bg-[#181b1f] rounded border border-[#2c2c2e]" ]
        [ div
            [ class "px-4 py-3 border-b border-[#2c2c2e] flex justify-between items-center" ]
            [ h2
                [ class "text-sm font-bold uppercase text-gray-400" ]
                [ text "Active Dispatchers (/api/dispatchers)" ]
            , span
                [ class "text-xs text-gray-500" ]
                [ text ("Total: " ++ String.fromInt (List.length dispatchers)) ]
            ]
        , div
            [ class "overflow-x-auto" ]
            [ table
                [ class "w-full text-left text-sm" ]
                [ thead
                    [ class "bg-[#222529] text-gray-300 uppercase text-xs" ]
                    [ tr []
                        [ th [ class "px-4 py-2" ] [ text "ID" ]
                        , th [ class "px-4 py-2" ] [ text "Status" ]
                        , th [ class "px-4 py-2" ] [ text "Last Seen" ]
                        , th [ class "px-4 py-2 text-right" ] [ text "Actions" ]
                        ]
                    ]
                , tbody
                    [ class "divide-y divide-[#2c2c2e]" ]
                    (List.map viewDispatcherRow dispatchers)
                ]
            ]
        , viewPagination
        ]


viewDispatcherRow : Dispatcher -> Html Msg
viewDispatcherRow dispatcher =
    tr
        [ class "hover:bg-[#222529] transition" ]
        [ td
            [ class "px-4 py-3 font-mono text-orange-400" ]
            [ text dispatcher.id ]
        , td
            [ class "px-4 py-3" ]
            [ span
                [ class (dispatcherStateClass dispatcher.state) ]
                [ text dispatcher.state
                ]
            ]
        , td
            [ class "px-4 py-3 text-gray-400" ]
            [ text dispatcher.lastSeen ]
        , td
            [ class "px-4 py-3 text-right" ]
            [ button
                [ class "text-blue-400 hover:underline mr-3" ]
                [ text "View" ]
            , button
                [ class "text-red-400 hover:underline" ]
                [ text "Suspend" ]
            ]
        ]


dispatcherStateClass : String -> String
dispatcherStateClass state =
    let
        base =
            "px-2 py-0.5 rounded border text-xs font-medium "
    in
    case String.toLower state of
        "running" ->
            base ++ "bg-green-900/30 text-green-400 border-green-800"

        "idle" ->
            base ++ "bg-yellow-900/30 text-yellow-400 border-yellow-800"

        "offline" ->
            base ++ "bg-red-900/30 text-red-400 border-red-800"

        _ ->
            base ++ "bg-gray-800 text-gray-400 border-gray-700"


viewPagination : Html Msg
viewPagination =
    div
        [ class "px-4 py-3 border-t border-[#2c2c2e] flex items-center justify-between text-sm" ]
        [ span
            [ class "text-gray-500" ]
            [ text "Showing 1–2 of 42" ]
        , div
            [ class "flex items-center gap-1" ]
            [ button
                [ class "px-2 py-1 rounded bg-[#222529] text-gray-400 hover:text-white hover:bg-[#2c2c2e] transition"
                , attribute "disabled" "true"
                ]
                [ text "Prev" ]
            , button
                [ class "px-2 py-1 rounded bg-orange-600 text-white font-medium" ]
                [ text "1" ]
            , button
                [ class "px-2 py-1 rounded bg-[#222529] text-gray-400 hover:text-white hover:bg-[#2c2c2e] transition" ]
                [ text "2" ]
            , button
                [ class "px-2 py-1 rounded bg-[#222529] text-gray-400 hover:text-white hover:bg-[#2c2c2e] transition" ]
                [ text "3" ]
            , button
                [ class "px-2 py-1 rounded bg-[#222529] text-gray-400 hover:text-white hover:bg-[#2c2c2e] transition" ]
                [ text "Next" ]
            ]
        ]


viewSummary : Html msg
viewSummary =
    div
        [ class "grid grid-cols-1 md:grid-cols-3 gap-4" ]
        [ div
            [ class "bg-[#181b1f] border-l-4 border-orange-500 p-4 rounded shadow-sm" ]
            [ p
                [ class "text-xs uppercase text-gray-400 font-bold mb-1" ]
                [ text "Total Devices" ]
            , p
                [ class "text-3xl font-mono text-white" ]
                [ text "1,284" ]
            ]
        , div
            [ class "bg-[#181b1f] border-l-4 border-purple-500 p-4 rounded shadow-sm" ]
            [ p
                [ class "text-xs uppercase text-gray-400 font-bold mb-1" ]
                [ text "Active Dispatchers" ]
            , p
                [ class "text-3xl font-mono text-white" ]
                [ text "42" ]
            ]
        , div
            [ class "bg-[#181b1f] border-l-4 border-green-500 p-4 rounded shadow-sm" ]
            [ p
                [ class "text-xs uppercase text-gray-400 font-bold mb-1" ]
                [ text "System Health" ]
            , p
                [ class "text-3xl font-mono text-green-400" ]
                [ text "99.8%" ]
            ]
        ]


navBar : Html Msg
navBar =
    nav
        [ class "bg-[#111217] border-b border-[#2c2c2e] px-4 py-2 flex items-center justify-between" ]
        [ div
            [ class "flex items-center gap-4" ]
            [ div
                [ class "bg-orange-500 p-1.5 rounded-sm" ]
                [ SvgAssets.logo
                ]
            , h1
                [ class "text-lg font-semibold tracking-tight" ]
                [ text "Ersha "
                , span
                    [ class "text-gray-500" ]
                    [ text "/ Registry" ]
                ]
            ]
        , div
            [ class "flex gap-3" ]
            [ button
                [ class "bg-[#2c2c2e] hover:bg-[#3a3a3c] px-3 py-1.5 rounded text-sm transition"
                , onClick FetchAll
                ]
                [ text "Refresh" ]
            , button
                [ class "bg-orange-600 hover:bg-orange-500 text-white px-3 py-1.5 rounded text-sm font-medium transition"
                , onClick <| OpenModal (DispatcherModal newDisparcherForm)
                ]
                [ text "Add Dispatcher" ]
            , button
                [ class "bg-green-600 hover:bg-green-500 text-white px-3 py-1.5 rounded text-sm font-medium transition" ]
                [ text "Add Device" ]
            ]
        ]


newDisparcherForm : DispatcherForm
newDisparcherForm =
    { id = Nothing
    , location = 0
    }


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        NoOp ->
            ( model, Cmd.none )

        OpenModal modalType ->
            ( { model | modal = modalType }, Cmd.none )

        CloseModal ->
            ( { model | modal = Closed }, Cmd.none )

        FetchAll ->
            ( { model | devices = Loading, dispatchers = Loading }
            , Cmd.batch [ getDispatchers, getDevices ]
            )

        GotDispatchers result ->
            case result of
                Ok data ->
                    ( { model | dispatchers = Success data }, Cmd.none )

                Err err ->
                    ( { model | dispatchers = Failure err }, Cmd.none )

        GotDevices result ->
            case result of
                Ok data ->
                    ( { model | devices = Success data }, Cmd.none )

                Err err ->
                    ( { model | devices = Failure err }, Cmd.none )

        SubmitForm modalState ->
            case modalState of
                DispatcherModal form ->
                    ( model, postDispatcher form )

                DeviceModal form ->
                    ( model, postDevice form )

                Closed ->
                    ( model, Cmd.none )

        SubmittedDispatcher result ->
            case result of
                Ok _ ->
                    -- Refresh list and close modal on success
                    ( { model | modal = Closed }, getDispatchers )

                Err err ->
                    -- In a real app, you might want to attach this error
                    -- specifically to the form state
                    ( { model | dispatchers = Failure err }, Cmd.none )

        SubmittedDevice result ->
            case result of
                Ok _ ->
                    ( { model | modal = Closed }, getDevices )

                Err err ->
                    ( { model | devices = Failure err }, Cmd.none )

        UpdateDispatcherForm form ->
            case model.modal of
                DispatcherModal _ ->
                    ( { model | modal = DispatcherModal form }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        _ ->
            ( model, Cmd.none )


httpErrorToString : Http.Error -> String
httpErrorToString error =
    case error of
        Http.BadUrl url ->
            "Invalid URL: " ++ url

        Http.Timeout ->
            "Server took too long to respond (Timeout)."

        Http.NetworkError ->
            "Network unreachable. Check your TLS certificates or connection."

        Http.BadStatus 401 ->
            "401: Unauthorized. mTLS handshake failed?"

        Http.BadStatus code ->
            "Server returned status code: " ++ String.fromInt code

        Http.BadBody message ->
            "Data conversion error: " ++ message


getDispatchers : Cmd Msg
getDispatchers =
    Http.get
        { url = "/api/dispatchers"
        , expect = Http.expectJson GotDispatchers (Decode.list dispatcherDecoder)
        }


getDevices : Cmd Msg
getDevices =
    Http.get
        { url = "/api/devices"
        , expect = Http.expectJson GotDevices (Decode.list deviceDecoder)
        }


postDispatcher : DispatcherForm -> Cmd Msg
postDispatcher form =
    Http.post
        { url = "/api/dispatchers"
        , body = Http.jsonBody (encodeDispatcherForm form)
        , expect = Http.expectWhatever SubmittedDispatcher
        }


postDevice : DeviceForm -> Cmd Msg
postDevice form =
    Http.post
        { url = "/api/devices"
        , body = Http.jsonBody (encodeDeviceForm form)
        , expect = Http.expectWhatever SubmittedDevice
        }


encodeDispatcherForm : DispatcherForm -> Encode.Value
encodeDispatcherForm form =
    Encode.object
        [ ( "id", Maybe.withDefault Encode.null (Maybe.map Encode.string form.id) )
        , ( "location", Encode.int form.location )
        ]


encodeDeviceForm : DeviceForm -> Encode.Value
encodeDeviceForm form =
    Encode.object
        [ ( "id", encodeMaybe Encode.string form.id )
        , ( "location", Encode.int form.location )
        , ( "kind", encodeMaybe encodeDeviceKind form.kind )
        , ( "manufacturer", encodeMaybe Encode.string form.manufacturer )
        , ( "sensors", Encode.list encodeSensorForm form.sensors )
        ]


encodeSensorForm : SensorForm -> Encode.Value
encodeSensorForm form =
    Encode.object
        [ ( "id", encodeMaybe Encode.string form.id )
        , ( "kind", encodeSensorKind form.kind )
        ]


encodeDeviceKind : DeviceKind -> Encode.Value
encodeDeviceKind kind =
    case kind of
        Sensor ->
            Encode.string "Sensor"


encodeSensorKind : SensorKind -> Encode.Value
encodeSensorKind kind =
    case kind of
        SoilMoisture ->
            Encode.string "SoilMoisture"

        SoilTemp ->
            Encode.string "SoilTemp"

        AirTemp ->
            Encode.string "AirTemp"

        Humidity ->
            Encode.string "Humidity"

        RainFall ->
            Encode.string "RainFall"


encodeMaybe : (a -> Encode.Value) -> Maybe a -> Encode.Value
encodeMaybe encoder maybeValue =
    case maybeValue of
        Just val ->
            encoder val

        Nothing ->
            Encode.null


deviceDecoder : Decoder Device
deviceDecoder =
    Decode.map6 Device
        (field "id" string)
        (field "kind" string)
        (field "state" string)
        (field "location" int)
        (maybe (field "manufacturer" string))
        (field "provisionedAt" string)


dispatcherDecoder : Decoder Dispatcher
dispatcherDecoder =
    Decode.map3 Dispatcher
        (field "id" string)
        (field "state" string)
        (field "lastSeen" string)


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.none


init : () -> ( Model, Cmd msg )
init _ =
    ( { devices = Success sampleDevices
      , dispatchers = Success sampleDispatchers
      , modal = Closed
      }
    , Cmd.none
    )


main : Program () Model Msg
main =
    Browser.document
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }


sampleDevices : List Device
sampleDevices =
    [ { id = "dev-x9"
      , kind = "gateway"
      , state = "online"
      , location = 617733123456789
      , manufacturer = Just "Helios Systems"
      , provisionedAt = "2025-01-12T09:42:18Z"
      }
    , { id = "dev-a1"
      , kind = "sensor-node"
      , state = "idle"
      , location = 617733123456790
      , manufacturer = Just "Axiom Devices"
      , provisionedAt = "2025-01-08T16:11:03Z"
      }
    , { id = "dev-k5"
      , kind = "edge-controller"
      , state = "offline"
      , location = 617733123456791
      , manufacturer = Nothing
      , provisionedAt = "2024-12-28T22:55:47Z"
      }
    ]


sampleDispatchers : List Dispatcher
sampleDispatchers =
    [ { id = "disp-8821"
      , state = "running"
      , lastSeen = "2s ago"
      }
    , { id = "disp-1044"
      , state = "idle"
      , lastSeen = "14m ago"
      }
    , { id = "disp-3390"
      , state = "offline"
      , lastSeen = "3h ago"
      }
    ]
