module Main exposing (main, sensorDecoder, subscriptions)

import Browser exposing (Document)
import Html exposing (..)
import Html.Attributes exposing (class, disabled, placeholder, selected, title, type_, value)
import Html.Events exposing (onClick, onInput)
import Http
import Json.Decode as Decode exposing (Decoder, fail, field, int, list, maybe, string, succeed)
import Json.Encode as Encode
import SvgAssets
import Url.Builder as Builder


type alias Model =
    { devices : ApiData ListDevicesResponse
    , dispatchers : ApiData ListDispatchersResponse
    , modal : Modals
    , devicePager : Pager
    , dispatcherPager : Pager
    }


type ApiData a
    = Loading
    | Failure Http.Error
    | Success a
    | NotAsked


type Modals
    = DispatcherModal DispatcherForm
    | DeviceModal DeviceForm
    | DetailDispatcherModal (ApiData Dispatcher)
    | DetailDeviceModal (ApiData Device)
    | Closed


type alias H3Cell =
    Int


type alias Ulid =
    String


type alias DispatcherForm =
    { id : Maybe Ulid
    , location : H3Cell
    }


type alias Dispatcher =
    { id : String
    , location : Int
    , state : String
    , provisionedAt : String
    }


type alias DeviceForm =
    { id : Maybe Ulid
    , location : H3Cell
    , kind : Maybe DeviceKind
    , manufacturer : Maybe String
    , sensors : List SensorForm
    }


type alias Device =
    { id : String
    , kind : String
    , state : String
    , location : Int
    , manufacturer : Maybe String
    , provisionedAt : String
    , sensors : List Sensor
    }


type DeviceKind
    = SensorDevice


type alias Sensor =
    { id : Ulid
    , kind : SensorKind
    }


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


type StateFilter
    = Active
    | Suspended


type SortOrder
    = Asc
    | Desc


type alias DispatchersQuery =
    { state : Maybe StateFilter
    , location : Maybe Int
    , sortOrder : Maybe SortOrder
    , offset : Maybe Int
    , limit : Maybe Int
    , after : Maybe String
    }


type alias ListDispatchersResponse =
    { dispatchers : List Dispatcher
    , total : Int
    }


type DeviceSortBy
    = SortState
    | SortManufacturer
    | SortSensorCount
    | SortProvisionedAt


type alias DevicesQuery =
    { state : Maybe StateFilter
    , location : Maybe Int
    , manufacturer : Maybe String
    , provisionedAfter : Maybe String
    , provisionedBefore : Maybe String
    , sortBy : Maybe DeviceSortBy
    , sortOrder : Maybe SortOrder
    , offset : Maybe Int
    , limit : Maybe Int
    , after : Maybe String
    }


type alias Pager =
    { currentPage : Int
    , itemsPerPage : Int
    , totalItems : Int
    }


type Msg
    = NoOp
      -- Navigation/UI
    | OpenModal Modals
    | CloseModal
      -- Forms
    | UpdateDispatcherForm DispatcherForm
    | UpdateDeviceForm DeviceForm
    | AddSensor
    | RemoveSensor Int
    | UpdateSensor Int SensorForm
      -- Api Lifecycle
    | FetchAll
    | GotDispatchers (Result Http.Error ListDispatchersResponse)
    | GotDevices (Result Http.Error ListDevicesResponse)
    | SubmittedDispatcher (Result Http.Error ())
    | SubmittedDevice (Result Http.Error ())
    | OpenDispatcherDetail Ulid
    | GotDetailDispatcher (Result Http.Error Dispatcher)
    | OpenDeviceDetail Ulid
    | GotDetailDevice (Result Http.Error Device)
    | SubmitForm Modals
      -- Pagination
    | SetDispatcherPage Int
    | SetDevicePage Int


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

                DeviceModal form ->
                    viewDeviceModal form

                DetailDispatcherModal data ->
                    viewDetailDispatcherModal data

                DetailDeviceModal data ->
                    viewDetailDeviceModal data

                Closed ->
                    text ""
            ]
        ]
    }


viewDetailDeviceModal : ApiData Device -> Html Msg
viewDetailDeviceModal remoteData =
    case remoteData of
        NotAsked ->
            text ""

        Loading ->
            viewModal
                { title = "Device Intelligence"
                , body =
                    div [ class "flex flex-col items-center justify-center p-12 space-y-4" ]
                        [ div [ class "w-8 h-8 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" ] []
                        , p [ class "text-sm text-gray-500 font-mono" ] [ text "Polling device registry..." ]
                        ]
                , footer = [ viewCloseButton ]
                }

        Failure err ->
            viewModal
                { title = "Registry Error"
                , body =
                    div [ class "bg-red-900/20 border border-red-500/50 p-6 rounded" ]
                        [ p [ class "text-red-400 text-sm font-mono" ] [ text (httpErrorToString err) ] ]
                , footer = [ viewCloseButton ]
                }

        Success device ->
            viewModal
                { title = "Device: " ++ device.id
                , body = viewDeviceDetails device
                , footer =
                    [ viewCloseButton
                    ]
                }


viewDeviceDetails : Device -> Html Msg
viewDeviceDetails device =
    div [ class "space-y-6" ]
        [ div [ class "grid grid-cols-3 gap-4" ]
            [ viewDetailItem "Status" (span [ class (stateBadgeClass device.state) ] [ text device.state ])
            , viewDetailItem "Hardware Kind" (text device.kind)
            , viewDetailItem "Manufacturer" (text (Maybe.withDefault "Generic/Custom" device.manufacturer))
            ]
        , div [ class "grid grid-cols-2 gap-4 bg-[#0b0c0e] p-4 rounded border border-[#2c2c2e]" ]
            [ viewDetailItem "H3 Location Index" (span [ class "text-orange-400 font-mono" ] [ text (String.fromInt device.location) ])
            , viewDetailItem "Provisioned At" (text device.provisionedAt)
            ]
        , div [ class "space-y-3" ]
            [ h4 [ class "text-xs font-bold text-gray-500 uppercase tracking-widest border-b border-[#2c2c2e] pb-2" ]
                [ text ("Attached Sensors (" ++ String.fromInt (List.length device.sensors) ++ ")") ]
            , if List.isEmpty device.sensors then
                p [ class "text-sm text-gray-600 italic" ] [ text "No sensors detected on this device." ]

              else
                div [ class "grid grid-cols-1 gap-2" ] (List.map viewSensorDetailItem device.sensors)
            ]
        ]


viewDetailItem : String -> Html Msg -> Html Msg
viewDetailItem txtLabel val =
    div [ class "flex flex-col gap-1" ]
        [ label [ class "text-[10px] text-gray-500 uppercase font-bold" ] [ text txtLabel ]
        , div [ class "text-sm text-[#d8d9da]" ] [ val ]
        ]


viewSensorDetailItem : Sensor -> Html Msg
viewSensorDetailItem sensor =
    div [ class "flex justify-between items-center bg-[#222529] px-4 py-3 rounded border border-[#2c2c2e] hover:border-gray-600 transition" ]
        [ div [ class "flex flex-col" ]
            [ span [ class "text-xs font-bold text-gray-300" ] [ text (sensorKindToString sensor.kind) ]
            , span [ class "text-[10px] text-gray-500 font-mono" ] [ text sensor.id ]
            ]
        , div [ class "h-2 w-2 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]" ] [] -- Vitality pulse
        ]


sensorKindToString : SensorKind -> String
sensorKindToString kind =
    case kind of
        SoilMoisture ->
            "Soil Moisture"

        SoilTemp ->
            "Soil Temperature"

        AirTemp ->
            "Air Temperature"

        Humidity ->
            "Humidity"

        RainFall ->
            "Rainfall Volume"


viewDetailDispatcherModal : ApiData Dispatcher -> Html Msg
viewDetailDispatcherModal remoteData =
    case remoteData of
        NotAsked ->
            text ""

        Loading ->
            viewModal
                { title = "Dispatcher Details"
                , body =
                    div [ class "flex flex-col items-center justify-center p-12 space-y-4" ]
                        [ div [ class "w-8 h-8 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" ] []
                        , p [ class "text-sm text-gray-500 font-mono" ] [ text "Fetching registry data..." ]
                        ]
                , footer = [ viewCloseButton ]
                }

        Failure err ->
            viewModal
                { title = "Error"
                , body =
                    div [ class "bg-red-900/20 border border-red-500/50 p-6 rounded" ]
                        [ p [ class "text-red-400 text-sm font-mono" ] [ text (httpErrorToString err) ] ]
                , footer = [ viewCloseButton ]
                }

        Success dispatcher ->
            viewModal
                { title = "Dispatcher: " ++ dispatcher.id
                , body = viewDispatcherDetails dispatcher
                , footer =
                    [ viewCloseButton
                    ]
                }


viewCloseButton : Html Msg
viewCloseButton =
    button
        [ class "text-[#d8d9da] hover:bg-[#2c2c2e] px-4 py-2 rounded text-sm transition-colors"
        , onClick CloseModal
        ]
        [ text "Close" ]


viewDispatcherDetails : Dispatcher -> Html Msg
viewDispatcherDetails dispatcher =
    div [ class "space-y-6" ]
        [ div [ class "flex items-center gap-4 bg-[#0b0c0e] p-4 rounded border border-[#2c2c2e]" ]
            [ div [ class "flex-1" ]
                [ label [ class "text-[10px] text-gray-500 uppercase font-bold" ] [ text "Current State" ]
                , div [ class "mt-1" ] [ span [ class (stateBadgeClass dispatcher.state) ] [ text dispatcher.state ] ]
                ]
            , div [ class "flex-1" ]
                [ label [ class "text-[10px] text-gray-500 uppercase font-bold" ] [ text "Provisioned At" ]
                , div [ class "text-sm text-[#d8d9da] mt-1 font-mono" ] [ text dispatcher.provisionedAt ]
                ]
            ]
        , div [ class "grid grid-cols-2 gap-6" ]
            [ div [ class "space-y-1" ]
                [ label [ class "text-[10px] text-orange-400 uppercase font-bold" ] [ text "Registry ID" ]
                , div [ class "text-sm text-[#d8d9da] font-mono break-all" ] [ text dispatcher.id ]
                ]
            , div [ class "space-y-1" ]
                [ label [ class "text-[10px] text-orange-400 uppercase font-bold" ] [ text "H3 Index (u64)" ]
                , div [ class "text-sm text-[#d8d9da] font-mono" ] [ text (String.fromInt dispatcher.location) ]
                ]
            ]
        , div [ class "pt-4 border-t border-[#2c2c2e]" ]
            [ h4 [ class "text-xs font-bold text-gray-500 uppercase mb-3" ] [ text "Network Performance" ]
            , div [ class "grid grid-cols-3 gap-2" ]
                [ viewMiniStat "Uptime" "99.8%"
                , viewMiniStat "Throughput" "1.2k/s"
                , viewMiniStat "Latency" "42ms"
                ]
            ]
        ]


viewMiniStat : String -> String -> Html Msg
viewMiniStat lbl val =
    div [ class "bg-[#222529] p-2 rounded border border-[#2c2c2e]" ]
        [ div [ class "text-[9px] text-gray-500 uppercase" ] [ text lbl ]
        , div [ class "text-xs font-bold text-white font-mono" ] [ text val ]
        ]


viewModal : { title : String, body : Html Msg, footer : List (Html Msg) } -> Html Msg
viewModal config =
    div [ class "fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm" ]
        [ div [ class "bg-[#181b1f] border border-[#2c2c2e] rounded-md shadow-2xl w-full max-w-2xl max-h-[90vh] flex flex-col" ]
            [ -- Header
              div [ class "bg-[#222529] px-6 py-4 border-b border-[#2c2c2e] flex justify-between items-center" ]
                [ h3 [ class "text-[#d8d9da] text-sm font-bold uppercase tracking-wider" ] [ text config.title ]
                , button [ class "text-gray-500 hover:text-white", onClick CloseModal ] [ text "✕" ]
                ]

            -- Body
            , div [ class "p-6 space-y-6 overflow-y-auto" ] [ config.body ]

            -- Footer
            , div [ class "bg-[#222529] px-6 py-4 flex justify-end gap-3 border-t border-[#2c2c2e]" ] config.footer
            ]
        ]


viewOptionalInput : String -> Maybe String -> String -> (Maybe String -> Msg) -> Html Msg
viewOptionalInput label current placeHolder toMsg =
    viewInput label
        (Maybe.withDefault "" current)
        placeHolder
        (\val ->
            toMsg
                (if val == "" then
                    Nothing

                 else
                    Just val
                )
        )


viewDeviceModal : DeviceForm -> Html Msg
viewDeviceModal form =
    viewModal
        { title = "Register New Device"
        , body =
            div [ class "space-y-6" ]
                [ div [ class "grid grid-cols-2 gap-4" ]
                    [ viewOptionalInput "Device ID (Optional)" form.id "Leave blank to auto-generate" (\id -> UpdateDeviceForm { form | id = id })
                    , viewInput "H3 Cell Index" (String.fromInt form.location) "" (\val -> UpdateDeviceForm { form | location = String.toInt val |> Maybe.withDefault 0 })
                    ]
                , div [ class "grid grid-cols-2 gap-4" ]
                    [ viewSelect "Manufacturer" (Maybe.withDefault "H3Cell Hexgon index" form.manufacturer) [ "Sony", "Bosch", "Ersha-Custom" ] (\val -> UpdateDeviceForm { form | manufacturer = Just val })
                    , viewSelect "Device Kind" "Sensor" [ "Sensor" ] (\_ -> UpdateDeviceForm { form | kind = Just SensorDevice })
                    ]
                , viewSensorSection form
                ]
        , footer =
            [ button [ class "text-[#d8d9da] hover:bg-[#2c2c2e] px-4 py-2 rounded text-sm", onClick CloseModal ] [ text "Cancel" ]
            , button [ class "bg-orange-600 hover:bg-orange-500 text-white px-4 py-2 rounded text-sm font-bold", onClick (SubmitForm (DeviceModal form)) ] [ text "Provision Device" ]
            ]
        }


viewSensorSection : DeviceForm -> Html Msg
viewSensorSection form =
    div [ class "space-y-4" ]
        [ div [ class "flex justify-between items-center border-b border-[#2c2c2e] pb-2" ]
            [ h4 [ class "text-xs font-bold text-gray-500 uppercase" ] [ text "Attached Sensors" ]
            , button [ class "text-xs bg-blue-600/20 text-blue-400 border border-blue-500/50 px-2 py-1 rounded", onClick AddSensor ] [ text "+ Add Sensor" ]
            ]
        , if List.isEmpty form.sensors then
            p [ class "text-xs text-gray-600 italic" ] [ text "No sensors added yet." ]

          else
            div [ class "space-y-3" ] (List.indexedMap viewSensorRow form.sensors)
        ]


viewInput : String -> String -> String -> (String -> Msg) -> Html Msg
viewInput labelText currentVal placeHoldr toMsg =
    div [ class "flex flex-col gap-1.5 w-full" ]
        [ label [ class "text-[10px] font-bold text-orange-400 uppercase tracking-tight" ]
            [ text labelText ]
        , input
            [ type_ "text"
            , class "bg-[#0b0c0e] border border-[#2c2c2e] text-[#d8d9da] rounded px-3 py-2 text-sm font-mono focus:border-orange-500 focus:outline-none transition-all placeholder:text-gray-700"
            , value currentVal
            , onInput toMsg
            , placeholder placeHoldr
            ]
            []
        ]


viewSelect : String -> String -> List String -> (String -> Msg) -> Html Msg
viewSelect labelText currentVal options toMsg =
    div [ class "flex flex-col gap-1.5 w-full" ]
        [ label [ class "text-[10px] font-bold text-orange-400 uppercase tracking-tight" ]
            [ text labelText ]
        , div [ class "relative" ]
            [ select
                [ class "w-full bg-[#0b0c0e] border border-[#2c2c2e] text-[#d8d9da] rounded px-3 py-2 text-sm appearance-none focus:border-orange-500 focus:outline-none cursor-pointer transition-all"
                , onInput toMsg
                ]
                (List.map (\opt -> option [ value opt, selected (opt == currentVal) ] [ text opt ]) options)
            , div [ class "absolute inset-y-0 right-3 flex items-center pointer-events-none text-gray-500 text-[10px]" ]
                [ text "▼" ]
            ]
        ]


viewSensorRow : Int -> SensorForm -> Html Msg
viewSensorRow index sensor =
    div [ class "flex flex-col gap-3 bg-[#0b0c0e] p-4 rounded border border-[#2c2c2e]" ]
        [ div [ class "flex items-start gap-3" ]
            [ div [ class "flex-1" ]
                [ viewInput "Sensor ID (Optional)"
                    (Maybe.withDefault "" sensor.id)
                    "Leave blank to auto generate"
                    (\val ->
                        UpdateSensor index
                            { sensor
                                | id =
                                    if val == "" then
                                        Nothing

                                    else
                                        Just val
                            }
                    )
                ]
            , button
                [ class "mt-6 bg-red-900/20 text-red-500 border border-red-900/50 p-2 rounded hover:bg-red-900/40 transition-colors"
                , onClick (RemoveSensor index)
                , title "Remove Sensor"
                ]
                [ text "🗑" ]
            ]
        , div [ class "flex-1" ]
            [ label [ class "text-[10px] text-gray-500 uppercase font-bold mb-1.5 block" ] [ text "Sensor Kind" ]
            , select
                [ class "w-full bg-[#181b1f] border border-[#2c2c2e] text-[#d8d9da] rounded px-2 py-1.5 text-xs focus:border-orange-500 focus:outline-none"
                , onInput (\val -> UpdateSensor index { sensor | kind = stringToSensorKind val })
                ]
                [ option [ value "soil_moisture", selected (sensor.kind == SoilMoisture) ] [ text "Soil Moisture" ]
                , option [ value "soil_temp", selected (sensor.kind == SoilTemp) ] [ text "Soil Temp" ]
                , option [ value "air_temp", selected (sensor.kind == AirTemp) ] [ text "Air Temp" ]
                , option [ value "humidity", selected (sensor.kind == Humidity) ] [ text "Humidity" ]
                , option [ value "rainfall", selected (sensor.kind == RainFall) ] [ text "Rainfall" ]
                ]
            ]
        ]


stringToSensorKind : String -> SensorKind
stringToSensorKind val =
    case val of
        "SoilMoisture" ->
            SoilMoisture

        "SoilTemp" ->
            SoilTemp

        "AirTemp" ->
            AirTemp

        "Humidity" ->
            Humidity

        "RainFall" ->
            RainFall

        _ ->
            SoilMoisture


viewDispatcherModal : DispatcherForm -> Html Msg
viewDispatcherModal form =
    viewModal
        { title = "Register Dispatcher"
        , body =
            div [ class "space-y-5" ]
                [ viewOptionalInput "Dispatcher ID (Optional)" form.id "Leave blank to auto-generate" (\id -> UpdateDispatcherForm { form | id = id })
                , viewInput "H3 Cell Index" (String.fromInt form.location) "H3Cell Hexagon index" (\val -> UpdateDispatcherForm { form | location = String.toInt val |> Maybe.withDefault 0 })
                ]
        , footer =
            [ button [ class "px-4 py-2 text-sm text-[#d8d9da] hover:bg-[#2c2c2e] rounded", onClick CloseModal ] [ text "Cancel" ]
            , button [ class "px-4 py-2 text-sm text-white bg-orange-600 rounded shadow-lg shadow-orange-900/20", onClick (SubmitForm (DispatcherModal form)) ] [ text "Register" ]
            ]
        }


mainContent : Model -> Html Msg
mainContent model =
    main_ [ class "p-6 space-y-6" ]
        [ viewSummary model
        , viewRemoteContent "Dispatchers" model.dispatchers <| viewDispatchers model.dispatcherPager
        , viewRemoteContent "Devices" model.devices <| viewDevices model.devicePager
        ]


viewDevices : Pager -> ListDevicesResponse -> Html Msg
viewDevices pager devices_resp =
    let
        devices =
            devices_resp.devices
    in
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
                        , th [ class "px-4 py-2 text-right" ] [ text "Actions" ]
                        ]
                    ]
                , tbody
                    [ class "divide-y divide-[#2c2c2e]" ]
                    (List.map viewDeviceRow devices)
                ]
            ]
        , viewPagination pager SetDevicePage
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
        , td
            [ class "px-4 py-3 text-right" ]
            [ button
                [ class "text-blue-400 hover:underline mr-3"
                , onClick <| OpenDeviceDetail device.id
                ]
                [ text "View" ]
            ]
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
                [ div [ class "w-8 h-8 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" ] []
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


viewPlaceholder : String -> Html Msg
viewPlaceholder msg =
    div [ class "p-12 border border-dashed border-[#2c2c2e] rounded text-center text-gray-600 italic text-sm" ]
        [ text msg ]


viewDispatchers : Pager -> ListDispatchersResponse -> Html Msg
viewDispatchers pager disp_resp =
    let
        dispatchers =
            disp_resp.dispatchers
    in
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
        , viewPagination pager SetDispatcherPage
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
            [ text dispatcher.provisionedAt ]
        , td
            [ class "px-4 py-3 text-right" ]
            [ button
                [ class "text-blue-400 hover:underline mr-3"
                , onClick <| OpenDispatcherDetail dispatcher.id
                ]
                [ text "View" ]
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


viewPagination : Pager -> (Int -> Msg) -> Html Msg
viewPagination pager toMsg =
    let
        totalPages =
            ceiling (toFloat pager.totalItems / toFloat pager.itemsPerPage)

        startItem =
            ((pager.currentPage - 1) * pager.itemsPerPage) + 1

        endItem =
            Basics.min (pager.currentPage * pager.itemsPerPage) pager.totalItems

        pages =
            List.range 1 totalPages
    in
    if pager.totalItems == 0 then
        text ""

    else
        div [ class "px-4 py-3 border-t border-[#2c2c2e] flex items-center justify-between text-sm" ]
            [ span [ class "text-gray-500" ]
                [ text ("Showing " ++ String.fromInt startItem ++ "–" ++ String.fromInt endItem ++ " of " ++ String.fromInt pager.totalItems) ]
            , div [ class "flex items-center gap-1" ]
                (viewPrevBtn pager toMsg :: List.map (viewPageBtn pager toMsg) pages ++ [ viewNextBtn pager totalPages toMsg ])
            ]


viewPageBtn : Pager -> (Int -> Msg) -> Int -> Html Msg
viewPageBtn pager toMsg pageNum =
    let
        isActive =
            pager.currentPage == pageNum
    in
    button
        [ class <|
            if isActive then
                "px-2 py-1 rounded bg-orange-600 text-white font-medium"

            else
                "px-2 py-1 rounded bg-[#222529] text-gray-400 hover:text-white hover:bg-[#2c2c2e] transition"
        , onClick (toMsg pageNum)
        , disabled isActive
        ]
        [ text (String.fromInt pageNum) ]


viewPrevBtn : Pager -> (Int -> Msg) -> Html Msg
viewPrevBtn pager toMsg =
    button
        [ class "px-2 py-1 rounded bg-[#222529] text-gray-400 disabled:opacity-30 hover:text-white transition"
        , disabled (pager.currentPage <= 1)
        , onClick (toMsg (pager.currentPage - 1))
        ]
        [ text "Prev" ]


viewNextBtn : Pager -> Int -> (Int -> Msg) -> Html Msg
viewNextBtn pager totalPages toMsg =
    button
        [ class "px-2 py-1 rounded bg-[#222529] text-gray-400 disabled:opacity-30 hover:text-white transition"
        , disabled (pager.currentPage >= totalPages)
        , onClick (toMsg (pager.currentPage + 1))
        ]
        [ text "Next" ]


viewSummary : Model -> Html msg
viewSummary model =
    div
        [ class "grid grid-cols-1 md:grid-cols-3 gap-4" ]
        [ div
            [ class "bg-[#181b1f] border-l-4 border-orange-500 p-4 rounded shadow-sm" ]
            [ p
                [ class "text-xs uppercase text-gray-400 font-bold mb-1" ]
                [ text "Total Devices" ]
            , p
                [ class "text-3xl font-mono text-white" ]
                [ text <| String.fromInt model.devicePager.totalItems ]
            ]
        , div
            [ class "bg-[#181b1f] border-l-4 border-purple-500 p-4 rounded shadow-sm" ]
            [ p
                [ class "text-xs uppercase text-gray-400 font-bold mb-1" ]
                [ text "Active Dispatchers" ]
            , p
                [ class "text-3xl font-mono text-white" ]
                [ text <| String.fromInt model.dispatcherPager.totalItems ]
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
                [ class "bg-green-600 hover:bg-green-500 text-white px-3 py-1.5 rounded text-sm font-medium transition"
                , onClick <| OpenModal (DeviceModal newDeviceModal)
                ]
                [ text "Add Device" ]
            ]
        ]


newDeviceModal : DeviceForm
newDeviceModal =
    { id = Nothing
    , location = 0
    , kind = Just SensorDevice
    , manufacturer = Just ""
    , sensors = []
    }


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
            , Cmd.batch [ getDispatchers defaultDispatchersQuery, getDevices defaultDevicesQuery ]
            )

        GotDispatchers result ->
            case result of
                Ok response ->
                    let
                        oldPager =
                            model.dispatcherPager

                        updatedPager =
                            { oldPager | totalItems = response.total }
                    in
                    ( { model | dispatchers = Success response, dispatcherPager = updatedPager }, Cmd.none )

                Err err ->
                    ( { model | dispatchers = Failure err }, Cmd.none )

        GotDevices result ->
            case result of
                Ok response ->
                    let
                        oldPager =
                            model.devicePager

                        updatedPager =
                            { oldPager | totalItems = response.total }
                    in
                    ( { model | devices = Success response, devicePager = updatedPager }, Cmd.none )

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

                DetailDispatcherModal _ ->
                    ( model, Cmd.none )

                DetailDeviceModal _ ->
                    ( model, Cmd.none )

        SubmittedDispatcher result ->
            case result of
                Ok _ ->
                    ( { model | modal = Closed }, getDispatchers defaultDispatchersQuery )

                Err err ->
                    ( { model | dispatchers = Failure err, modal = Closed }, Cmd.none )

        SubmittedDevice result ->
            case result of
                Ok _ ->
                    ( { model | modal = Closed }, getDevices defaultDevicesQuery )

                Err err ->
                    ( { model | devices = Failure err, modal = Closed }, Cmd.none )

        UpdateDispatcherForm form ->
            case model.modal of
                DispatcherModal _ ->
                    ( { model | modal = DispatcherModal form }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        AddSensor ->
            case model.modal of
                DeviceModal form ->
                    let
                        newSensor =
                            { id = Nothing, kind = SoilMoisture }

                        updatedForm =
                            { form | sensors = newSensor :: form.sensors }
                    in
                    ( { model | modal = DeviceModal updatedForm }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        RemoveSensor index ->
            case model.modal of
                DeviceModal form ->
                    let
                        updatedSensors =
                            List.take index form.sensors ++ List.drop (index + 1) form.sensors

                        updatedForm =
                            { form | sensors = updatedSensors }
                    in
                    ( { model | modal = DeviceModal updatedForm }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        UpdateSensor index updatedSensor ->
            case model.modal of
                DeviceModal form ->
                    let
                        updateEntry i old =
                            if i == index then
                                updatedSensor

                            else
                                old

                        updatedSensors =
                            List.indexedMap updateEntry form.sensors

                        updatedForm =
                            { form | sensors = updatedSensors }
                    in
                    ( { model | modal = DeviceModal updatedForm }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        UpdateDeviceForm form ->
            case model.modal of
                DeviceModal _ ->
                    ( { model | modal = DeviceModal form }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        OpenDispatcherDetail dispatcher_id ->
            ( { model | modal = DetailDispatcherModal Loading }, getDispatcher dispatcher_id )

        GotDetailDispatcher result ->
            case result of
                Ok response ->
                    ( { model | modal = DetailDispatcherModal (Success response) }, Cmd.none )

                Err err ->
                    ( { model | modal = DetailDispatcherModal (Failure err) }, Cmd.none )

        OpenDeviceDetail device_id ->
            ( { model | modal = DetailDeviceModal Loading }, getDevice device_id )

        GotDetailDevice result ->
            case result of
                Ok response ->
                    ( { model | modal = DetailDeviceModal (Success response) }, Cmd.none )

                Err err ->
                    ( { model | modal = DetailDeviceModal (Failure err) }, Cmd.none )

        SetDevicePage pageNum ->
            let
                oldPager =
                    model.devicePager

                newPager =
                    { oldPager | currentPage = pageNum }

                query =
                    { defaultDevicesQuery | offset = Just (toOffset newPager), limit = Just newPager.itemsPerPage }
            in
            ( { model | devicePager = newPager }, getDevices query )

        SetDispatcherPage pageNum ->
            let
                oldPager =
                    model.devicePager

                newPager =
                    { oldPager | currentPage = pageNum }

                query =
                    { defaultDispatchersQuery | offset = Just (toOffset newPager), limit = Just newPager.itemsPerPage }
            in
            ( { model | dispatcherPager = newPager }, getDispatchers query )


getDevice : Ulid -> Cmd Msg
getDevice device_id =
    Http.get
        { url = "/api/devices/" ++ device_id
        , expect = Http.expectJson GotDetailDevice deviceDecoder
        }


getDispatcher : Ulid -> Cmd Msg
getDispatcher dispatcher_id =
    Http.get
        { url = "/api/dispatchers/" ++ dispatcher_id
        , expect = Http.expectJson GotDetailDispatcher dispatcherDecoder
        }


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


getDispatchers : DispatchersQuery -> Cmd Msg
getDispatchers query =
    let
        params =
            [ query.state
                |> Maybe.map (\s -> Builder.string "state" (stateFilterToString s))
            , query.location
                |> Maybe.map (\l -> Builder.int "location" l)
            , query.sortOrder
                |> Maybe.map (\o -> Builder.string "sort_order" (sortOrderToString o))
            , query.offset
                |> Maybe.map (\n -> Builder.int "offset" n)
            , query.limit
                |> Maybe.map (\n -> Builder.int "limit" n)
            , query.after
                |> Maybe.map (\a -> Builder.string "after" a)
            ]
                |> List.filterMap identity

        -- Remove the Nothings
        apiUrl =
            Builder.relative [ "api", "dispatchers" ] params
    in
    Http.get
        { url = apiUrl
        , expect = Http.expectJson GotDispatchers dispatchersResponseDecoder
        }



-- Helpers


stateFilterToString : StateFilter -> String
stateFilterToString s =
    case s of
        Active ->
            "active"

        Suspended ->
            "suspended"


sortOrderToString : SortOrder -> String
sortOrderToString o =
    case o of
        Asc ->
            "asc"

        Desc ->
            "desc"


dispatchersResponseDecoder : Decoder ListDispatchersResponse
dispatchersResponseDecoder =
    Decode.map2 ListDispatchersResponse
        (field "dispatchers" (list dispatcherDecoder))
        (field "total" int)


getDevices : DevicesQuery -> Cmd Msg
getDevices query =
    let
        params =
            [ query.state |> Maybe.map (\s -> Builder.string "state" (stateFilterToString s))
            , query.location |> Maybe.map (\l -> Builder.int "location" l)
            , query.manufacturer |> Maybe.map (\m -> Builder.string "manufacturer" m)
            , query.provisionedAfter |> Maybe.map (\ts -> Builder.string "provisioned_after" ts)
            , query.provisionedBefore |> Maybe.map (\ts -> Builder.string "provisioned_before" ts)
            , query.sortBy |> Maybe.map (\b -> Builder.string "sort_by" (deviceSortToString b))
            , query.sortOrder |> Maybe.map (\o -> Builder.string "sort_order" (sortOrderToString o))
            , query.offset |> Maybe.map (\n -> Builder.int "offset" n)
            , query.limit |> Maybe.map (\n -> Builder.int "limit" n)
            , query.after |> Maybe.map (\a -> Builder.string "after" a)
            ]
                |> List.filterMap identity

        apiUrl =
            Builder.relative [ "api", "devices" ] params
    in
    Http.get
        { url = apiUrl
        , expect = Http.expectJson GotDevices devicesResponseDecoder
        }


deviceSortToString : DeviceSortBy -> String
deviceSortToString b =
    case b of
        SortState ->
            "state"

        SortManufacturer ->
            "manufacturer"

        SortSensorCount ->
            "sensorCount"

        SortProvisionedAt ->
            "provisionedAt"


type alias ListDevicesResponse =
    { devices : List Device
    , total : Int
    }


devicesResponseDecoder : Decoder ListDevicesResponse
devicesResponseDecoder =
    Decode.map2 ListDevicesResponse
        (field "devices" (list deviceDecoder))
        (field "total" int)


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
        SensorDevice ->
            Encode.string "Sensor"


encodeSensorKind : SensorKind -> Encode.Value
encodeSensorKind kind =
    case kind of
        SoilMoisture ->
            Encode.string "soil_moisture"

        SoilTemp ->
            Encode.string "soil_temp"

        AirTemp ->
            Encode.string "air_temp"

        Humidity ->
            Encode.string "humidity"

        RainFall ->
            Encode.string "rainfall"


encodeMaybe : (a -> Encode.Value) -> Maybe a -> Encode.Value
encodeMaybe encoder maybeValue =
    case maybeValue of
        Just val ->
            encoder val

        Nothing ->
            Encode.null


deviceDecoder : Decoder Device
deviceDecoder =
    Decode.map7 Device
        (field "id" string)
        (field "kind" string)
        (field "state" string)
        (field "location" int)
        (maybe (field "manufacturer" string))
        (field "provisioned_at" string)
        (field "sensors" (list sensorDecoder))


sensorDecoder : Decoder Sensor
sensorDecoder =
    Decode.map2 Sensor
        (field "id" string)
        (field "kind" decodeSensorKind)


decodeSensorKind : Decoder SensorKind
decodeSensorKind =
    Decode.andThen stringToSensorKindDecoder string


stringToSensorKindDecoder : String -> Decoder SensorKind
stringToSensorKindDecoder val =
    case val of
        "soil_moisture" ->
            succeed SoilMoisture

        "soil_temp" ->
            succeed SoilTemp

        "air_temp" ->
            succeed AirTemp

        "humidity" ->
            succeed Humidity

        "rainfall" ->
            succeed RainFall

        other ->
            fail <| "Trying to decode sensor kind, but " ++ other ++ " , is not no known"


dispatcherDecoder : Decoder Dispatcher
dispatcherDecoder =
    Decode.map4 Dispatcher
        (field "id" string)
        (field "location" int)
        (field "state" string)
        (field "provisioned_at" string)


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.none


init : () -> ( Model, Cmd Msg )
init _ =
    ( { devices = Loading
      , dispatchers = Loading
      , modal = Closed
      , devicePager = initPager 50
      , dispatcherPager = initPager 50
      }
    , Cmd.batch [ getDispatchers defaultDispatchersQuery, getDevices defaultDevicesQuery ]
    )


main : Program () Model Msg
main =
    Browser.document
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }


defaultDevicesQuery : DevicesQuery
defaultDevicesQuery =
    { state = Nothing
    , manufacturer = Nothing
    , provisionedAfter = Nothing
    , provisionedBefore = Nothing
    , sortBy = Nothing
    , sortOrder = Just Desc
    , limit = Just 50
    , after = Nothing
    , location = Nothing
    , offset = Just 0
    }


defaultDispatchersQuery : DispatchersQuery
defaultDispatchersQuery =
    { state = Nothing
    , location = Nothing
    , sortOrder = Just Desc
    , offset = Just 0
    , limit = Just 50
    , after = Nothing
    }


initPager : Int -> Pager
initPager limit =
    { currentPage = 1
    , itemsPerPage = limit
    , totalItems = 0
    }


toOffset : Pager -> Int
toOffset pager =
    (pager.currentPage - 1) * pager.itemsPerPage
