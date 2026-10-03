import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

import com.openscq30.guiqt 1.0

ApplicationWindow {
    id: root
    width: 720
    height: 600
    visible: true
    title: qsTr("OpenSCQ30 (Qt)")

    SessionManager {
        id: session
    }

    Component.onCompleted: {
        session.loadDeviceModels()
        session.refreshPairedDevices()
    }

    function parseList(text) {
        try {
            const v = JSON.parse(text || "[]")
            return Array.isArray(v) ? v : []
        } catch (e) {
            return []
        }
    }

    function valuePayload(type, value) {
        return JSON.stringify({ type: type, value: value })
    }

    Connections {
        target: session
        function onConnectionStatusChanged() {
            if (session.connectionStatus === "Connected" && stack.depth === 1) {
                session.loadLegacyProfiles()
                stack.push(settingsPage, StackView.Immediate)
            } else if ((session.connectionStatus === "Disconnected"
                        || session.connectionStatus === "Failed") && stack.depth > 1) {
                stack.pop(StackView.Immediate)
            }
        }
    }

    StackView {
        id: stack
        anchors.fill: parent
        initialItem: selectionPage
    }

    // ---- Device selection -------------------------------------------------
    Component {
        id: selectionPage
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12

            Label {
                text: qsTr("Paired devices")
                font.bold: true
                font.pointSize: 16
            }
            Label {
                text: session.status
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Label {
                text: session.errorMessage
                visible: session.errorMessage.length > 0
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                color: "red"
            }
            RowLayout {
                Button {
                    text: qsTr("Refresh")
                    icon.name: "view-refresh"
                    onClicked: session.refreshPairedDevices()
                }
                Button {
                    text: qsTr("Add device")
                    icon.name: "list-add"
                    onClicked: stack.push(addPage, StackView.Immediate)
                }
                Label {
                    text: qsTr("%1 device(s)").arg(session.pairedCount)
                }
            }
            ListView {
                id: deviceList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 8
                model: root.parseList(session.pairedDevicesJson)
                delegate: Frame {
                    width: deviceList.width
                    RowLayout {
                        width: parent.width
                        spacing: 8
                        ColumnLayout {
                            Layout.fillWidth: true
                            Label {
                                text: modelData.modelName
                                font.bold: true
                            }
                            Label {
                                text: modelData.mac
                                font.pointSize: 9
                                opacity: 0.7
                            }
                        }
                        Button {
                            text: qsTr("Remove")
                            icon.name: "edit-delete"
                            onClicked: session.unpairDevice(modelData.mac)
                        }
                        Button {
                            text: session.connectionStatus === "Connecting..."
                                  ? qsTr("Connecting...") : qsTr("Connect")
                            enabled: session.connectionStatus !== "Connecting..."
                            onClicked: session.connectToDevice(modelData.mac)
                        }
                    }
                }
            }
            Label {
                visible: deviceList.count === 0
                text: qsTr("No paired devices yet. Add a device to get started.")
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                opacity: 0.6
            }
        }
    }

    // ---- Add device -------------------------------------------------------
    Component {
        id: addPage
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12

            Label {
                text: qsTr("Add device")
                font.bold: true
                font.pointSize: 16
            }
            RowLayout {
                Layout.fillWidth: true
                Label { text: qsTr("Model:") }
                ComboBox {
                    id: modelPicker
                    Layout.fillWidth: true
                    textRole: "name"
                    valueRole: "id"
                    model: root.parseList(session.deviceModelsJson)
                }
            }
            RowLayout {
                Button {
                    text: qsTr("Back")
                    icon.name: "go-previous"
                    onClicked: stack.pop(StackView.Immediate)
                }
                Button {
                    text: qsTr("Scan")
                    icon.name: "system-search"
                    enabled: modelPicker.count > 0
                    onClicked: session.startScan(modelPicker.currentValue)
                }
                Label {
                    text: session.scanStatus
                }
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 8
                model: root.parseList(session.scanResultsJson)
                delegate: Frame {
                    width: parent.width
                    RowLayout {
                        width: parent.width
                        spacing: 8
                        ColumnLayout {
                            Layout.fillWidth: true
                            Label {
                                text: modelData.name
                                font.bold: true
                            }
                            Label {
                                text: modelData.macAddress
                                font.pointSize: 9
                                opacity: 0.7
                            }
                        }
                        Button {
                            text: qsTr("Pair")
                            onClicked: {
                                session.pairDevice(modelData.macAddress,
                                                   modelPicker.currentValue)
                                stack.pop(StackView.Immediate)
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- Device settings --------------------------------------------------
    Component {
        id: settingsPage
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Button {
                    text: qsTr("Back")
                    icon.name: "go-previous"
                    onClicked: session.disconnectDevice()
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    Label {
                        text: session.connectedModelName
                        font.bold: true
                        font.pointSize: 14
                    }
                    Label {
                        text: session.connectedMac + " · " + session.connectionStatus
                        font.pointSize: 9
                        opacity: 0.7
                    }
                }
                ComboBox {
                    id: categoryPicker
                    Layout.preferredWidth: 200
                    textRole: "name"
                    valueRole: "id"
                    model: root.parseList(session.categoriesJson)
                    currentIndex: {
                        const cats = root.parseList(session.categoriesJson)
                        const i = cats.findIndex(c => c.id === session.activeCategory)
                        return i >= 0 ? i : 0
                    }
                    onActivated: (i) => {
                        const cats = root.parseList(session.categoriesJson)
                        if (i >= 0 && i < cats.length)
                            session.selectCategory(cats[i].id)
                    }
                }
                Button {
                    text: qsTr("Presets")
                    onClicked: {
                        session.loadQuickPresets()
                        stack.push(presetsPage, StackView.Immediate)
                    }
                }
            }
            Label {
                text: session.errorMessage
                visible: session.errorMessage.length > 0
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                color: "red"
            }
            Frame {
                visible: root.parseList(session.legacyProfilesJson).length > 0
                Layout.fillWidth: true
                ColumnLayout {
                    width: parent.width
                    spacing: 4
                    Label {
                        text: qsTr("Legacy equalizer profiles (v1 config found)")
                        font.bold: true
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                    Repeater {
                        model: root.parseList(session.legacyProfilesJson)
                        RowLayout {
                            Layout.fillWidth: true
                            property var profile: modelData
                            Label {
                                Layout.fillWidth: true
                                text: profile.name + "  [" + profile.values.join(", ") + "]"
                                wrapMode: Text.Wrap
                                font.pointSize: 9
                            }
                            Button {
                                text: qsTr("Migrate")
                                onClicked: session.migrateLegacyProfile(profile.name)
                            }
                        }
                    }
                }
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 4
                model: root.parseList(session.settingsJson)
                delegate: Frame {
                    width: parent.width
                    ColumnLayout {
                        width: parent.width
                        spacing: 4
                        Label {
                            text: modelData.name
                            font.bold: true
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                        }
                        Label {
                            text: modelData.displayValue
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            opacity: 0.7
                            font.pointSize: 9
                            visible: modelData.kind !== "info" && modelData.kind !== "action"
                        }

                        // toggle
                        Switch {
                            visible: modelData.kind === "toggle"
                            property var entry: modelData
                            enabled: entry.mode !== "readOnly"
                            checked: entry.value === true
                            onToggled: session.setSettingValue(
                                entry.id, root.valuePayload("bool", checked))
                        }

                        // i32 range
                        RowLayout {
                            visible: modelData.kind === "range"
                            property var entry: modelData
                            property int pending: entry.value
                            enabled: entry.mode !== "readOnly"
                            Label { text: entry.min }
                            Slider {
                                Layout.fillWidth: true
                                from: entry.min
                                to: entry.max
                                stepSize: entry.step
                                value: pending
                                onMoved: {
                                    pending = Math.round(value)
                                    rangeDeb.restart()
                                }
                            }
                            Label { text: pending }
                            Timer {
                                id: rangeDeb
                                interval: 250
                                onTriggered: session.setSettingValue(
                                    entry.id, root.valuePayload("i32", pending))
                            }
                        }

                        // select / optional / modifiable
                        ColumnLayout {
                            visible: modelData.kind === "select"
                            property var entry: modelData
                            ComboBox {
                                Layout.fillWidth: true
                                enabled: entry.mode !== "readOnly"
                                textRole: "label"
                                model: {
                                    const items = []
                                    for (let i = 0; i < entry.options.length; i++)
                                        items.push({ label: entry.localizedOptions[i],
                                                     opt: entry.options[i] })
                                    if (entry.allowNone)
                                        items.push({ label: qsTr("None"), opt: null })
                                    return items
                                }
                                currentIndex: {
                                    const i = entry.options.indexOf(entry.value)
                                    if (i >= 0) return i
                                    return entry.allowNone ? entry.options.length : 0
                                }
                                onActivated: (i) => {
                                    const opt = i < entry.options.length
                                                ? entry.options[i] : null
                                    const t = (entry.modifiable || entry.allowNone)
                                              ? "optionalString" : "string"
                                    session.setSettingValue(entry.id, root.valuePayload(t, opt))
                                }
                            }
                            RowLayout {
                                visible: entry.modifiable && entry.mode !== "readOnly"
                                TextField {
                                    id: modField
                                    Layout.fillWidth: true
                                    placeholderText: qsTr("New value")
                                }
                                Button {
                                    text: qsTr("Add")
                                    onClicked: session.setSettingValue(entry.id, JSON.stringify({
                                        type: "modifiableSelectCommand",
                                        value: { type: "add", name: modField.text }
                                    }))
                                }
                                Button {
                                    text: qsTr("Remove current")
                                    enabled: entry.value !== null
                                    onClicked: session.setSettingValue(entry.id, JSON.stringify({
                                        type: "modifiableSelectCommand",
                                        value: { type: "remove", name: entry.value }
                                    }))
                                }
                            }
                        }

                        // multi select
                        ColumnLayout {
                            visible: modelData.kind === "multi"
                            property var entry: modelData
                            Repeater {
                                model: entry.options
                                RowLayout {
                                    property string opt: modelData
                                    property int optIndex: index
                                    CheckBox {
                                        enabled: entry.mode !== "readOnly"
                                        text: entry.localizedOptions[optIndex]
                                        checked: entry.values.indexOf(opt) >= 0
                                        onToggled: {
                                            let next = entry.values.slice()
                                            const i = next.indexOf(opt)
                                            if (checked && i < 0) next.push(opt)
                                            if (!checked && i >= 0) next.splice(i, 1)
                                            session.setSettingValue(
                                                entry.id, root.valuePayload("stringVec", next))
                                        }
                                    }
                                    Button {
                                        text: qsTr("Remove")
                                        visible: entry.removable && entry.mode !== "readOnly"
                                        onClicked: session.setSettingValue(entry.id, JSON.stringify({
                                            type: "multiSelectWithRemoveCommand",
                                            value: { type: "remove", name: opt }
                                        }))
                                    }
                                }
                            }
                        }

                        // equalizer
                        ColumnLayout {
                            visible: modelData.kind === "equalizer"
                            property var entry: modelData
                            property var pending: entry.value.slice()
                            enabled: entry.mode !== "readOnly"
                            Timer {
                                id: eqDeb
                                interval: 300
                                onTriggered: session.setSettingValue(
                                    entry.id, root.valuePayload("i16Vec", pending))
                            }
                            Repeater {
                                model: pending.length
                                RowLayout {
                                    property int band: index
                                    Label {
                                        text: entry.bandHz[band] + " Hz"
                                        Layout.preferredWidth: 70
                                    }
                                    Slider {
                                        Layout.fillWidth: true
                                        from: entry.min
                                        to: entry.max
                                        stepSize: 1
                                        value: pending[band]
                                        onMoved: {
                                            const next = pending.slice()
                                            next[band] = Math.round(value)
                                            pending = next
                                            eqDeb.restart()
                                        }
                                    }
                                    Label {
                                        text: (pending[band] / Math.pow(10, entry.fractionDigits))
                                              .toFixed(entry.fractionDigits) + " dB"
                                        Layout.preferredWidth: 70
                                    }
                                }
                            }
                        }

                        // preset equalizer profile
                        ComboBox {
                            visible: modelData.kind === "presetEqualizer"
                            property var entry: modelData
                            Layout.fillWidth: true
                            enabled: entry.mode !== "readOnly"
                            model: entry.localizedOptions
                            currentIndex: Math.max(0, entry.options.indexOf(entry.value))
                            onActivated: (i) => session.setSettingValue(
                                entry.id, root.valuePayload("optionalString", entry.options[i]))
                        }

                        // information
                        Label {
                            visible: modelData.kind === "info"
                            property var entry: modelData
                            text: entry.displayValue
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }

                        // import string
                        ColumnLayout {
                            visible: modelData.kind === "import"
                            property var entry: modelData
                            TextField {
                                id: importField
                                Layout.fillWidth: true
                                enabled: entry.mode !== "readOnly"
                                placeholderText: qsTr("Paste export string")
                            }
                            Label {
                                text: entry.hint || ""
                                wrapMode: Text.Wrap
                                Layout.fillWidth: true
                                opacity: 0.7
                                font.pointSize: 9
                                visible: (entry.hint || "").length > 0
                            }
                            Button {
                                property bool armed: false
                                text: armed ? qsTr("Click again to confirm") : qsTr("Import")
                                enabled: entry.mode !== "readOnly" && importField.text.length > 0
                                onClicked: {
                                    if (!armed) { armed = true; return }
                                    armed = false
                                    session.setSettingValue(
                                        entry.id, root.valuePayload("string", importField.text))
                                }
                            }
                        }

                        // hue color picker
                        RowLayout {
                            visible: modelData.kind === "hue"
                            property var entry: modelData
                            property real pendingHue: entry.value
                            enabled: entry.mode !== "readOnly"
                            Label { text: "0°" }
                            Slider {
                                Layout.fillWidth: true
                                from: 0
                                to: 360
                                stepSize: 1
                                value: pendingHue
                                onMoved: {
                                    pendingHue = value
                                    hueDeb.restart()
                                }
                            }
                            Label { text: "360°" }
                            Timer {
                                id: hueDeb
                                interval: 250
                                onTriggered: session.setSettingValue(
                                    entry.id, root.valuePayload("f32", pendingHue))
                            }
                        }

                        // action
                        Button {
                            visible: modelData.kind === "action"
                            property var entry: modelData
                            text: qsTr("Execute")
                            icon.name: "system-run"
                            enabled: entry.mode !== "readOnly"
                            onClicked: session.setSettingValue(
                                entry.id, root.valuePayload("bool", false))
                        }

                        // time of day
                        RowLayout {
                            visible: modelData.kind === "time"
                            property var entry: modelData
                            property int total: entry.value
                            enabled: entry.mode !== "readOnly"
                            SpinBox {
                                id: hhBox
                                from: 0
                                to: 23
                                value: Math.floor(total / 60)
                            }
                            Label { text: ":" }
                            SpinBox {
                                id: mmBox
                                from: 0
                                to: 59
                                value: total % 60
                            }
                            Button {
                                text: qsTr("Set")
                                onClicked: session.setSettingValue(entry.id, root.valuePayload(
                                    "i32", hhBox.value * 60 + mmBox.value))
                            }
                        }

                        // fallback (should never show: Rust maps every variant)
                        Label {
                            visible: ["toggle", "range", "select", "multi", "equalizer",
                                      "presetEqualizer", "info", "import", "hue",
                                      "action", "time"].indexOf(modelData.kind) < 0
                            text: qsTr("Unsupported setting: %1").arg(modelData.kind)
                            opacity: 0.6
                        }
                    }
                }
            }
        }
    }

    // ---- Quick presets ----------------------------------------------------
    Component {
        id: presetsPage
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 8
            property string editingPreset: ""
            property string pendingDelete: ""

            Component.onCompleted: session.loadQuickPresets()

            RowLayout {
                Layout.fillWidth: true
                Button {
                    text: qsTr("Back")
                    icon.name: "go-previous"
                    onClicked: stack.pop(StackView.Immediate)
                }
                Label {
                    text: qsTr("Quick presets")
                    font.bold: true
                    font.pointSize: 16
                    Layout.fillWidth: true
                }
            }
            Label {
                text: session.errorMessage
                visible: session.errorMessage.length > 0
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                color: "red"
            }
            RowLayout {
                Layout.fillWidth: true
                TextField {
                    id: presetNameField
                    Layout.fillWidth: true
                    placeholderText: qsTr("New preset name")
                }
                Button {
                    text: qsTr("Create")
                    icon.name: "list-add"
                    enabled: presetNameField.text.trim().length > 0
                    onClicked: {
                        session.createQuickPreset(presetNameField.text.trim())
                        presetNameField.text = ""
                    }
                }
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 4
                model: root.parseList(session.quickPresetsJson)
                delegate: Frame {
                    width: parent.width
                    property var preset: modelData
                    ColumnLayout {
                        width: parent.width
                        spacing: 4
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: preset.name
                                font.bold: true
                                Layout.fillWidth: true
                            }
                            Button {
                                text: qsTr("Activate")
                                onClicked: session.activateQuickPreset(preset.name)
                            }
                            Button {
                                text: editingPreset === preset.name ? qsTr("Close") : qsTr("Edit")
                                onClicked: editingPreset = editingPreset === preset.name
                                                          ? "" : preset.name
                            }
                            Button {
                                text: pendingDelete === preset.name
                                      ? qsTr("Confirm delete") : qsTr("Delete")
                                icon.name: "edit-delete"
                                onClicked: {
                                    if (pendingDelete === preset.name) {
                                        pendingDelete = ""
                                        session.deleteQuickPreset(preset.name)
                                        if (editingPreset === preset.name)
                                            editingPreset = ""
                                    } else {
                                        pendingDelete = preset.name
                                    }
                                }
                            }
                        }
                        ColumnLayout {
                            visible: editingPreset === preset.name
                            Layout.fillWidth: true
                            spacing: 2
                            Repeater {
                                model: preset.fields
                                RowLayout {
                                    Layout.fillWidth: true
                                    property var field: modelData
                                    CheckBox {
                                        checked: field.isEnabled
                                        onToggled: session.togglePresetField(
                                            preset.name, field.settingId, checked)
                                    }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        Label {
                                            text: field.name
                                            font.pointSize: 9
                                            opacity: checked ? 1.0 : 0.5
                                        }
                                        Label {
                                            text: field.displayValue
                                            font.pointSize: 9
                                            opacity: 0.6
                                            wrapMode: Text.Wrap
                                            Layout.fillWidth: true
                                        }
                                    }
                                }
                            }
                            Button {
                                text: qsTr("Overwrite with current settings")
                                onClicked: session.snapshotPreset(preset.name)
                            }
                        }
                    }
                }
            }
        }
    }
}
