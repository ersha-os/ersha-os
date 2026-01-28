module Main exposing (main, subscriptions)

import Browser exposing (Document)
import Html exposing (..)
import Html.Attributes exposing (attribute, class)
import SvgAssets
import Types exposing (Device, Dispatcher)


type alias Model =
    { devices : List Device
    , dispatchers : List Dispatcher
    }


type Msg
    = NoOp


view : Model -> Document Msg
view model =
    { title = "ersha-registry"
    , body =
        [ div [ class "min-h-screen bg-[#0b0c0e] text-[#d8d9da] font-sans" ]
            [ navBar
            , mainContent model
            ]
        ]
    }


mainContent : Model -> Html Msg
mainContent model =
    main_ [ class "p-6 space-y-6" ]
        [ viewSummary
        , viewDispatchers model.dispatchers
        , viewDevices model.devices
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


navBar : Html msg
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
                [ class "bg-[#2c2c2e] hover:bg-[#3a3a3c] px-3 py-1.5 rounded text-sm transition" ]
                [ text "Refresh" ]
            , button
                [ class "bg-orange-600 hover:bg-orange-500 text-white px-3 py-1.5 rounded text-sm font-medium transition" ]
                [ text "Add Dispatcher" ]
            , button
                [ class "bg-green-600 hover:bg-orange-500 text-white px-3 py-1.5 rounded text-sm font-medium transition" ]
                [ text "Add Device" ]
            ]
        ]


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
    ( { devices = sampleDevices, dispatchers = sampleDispatchers }, Cmd.none )


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
