<template>
    <div class="node-device-container">
        <h2 class="node-device-name">{{ deviceName }} - <span class="node-device-type">{{ deviceMetadata.device_type }}</span></h2>
        <transition-group name="device-list" tag="div" class="device-sensors-container">
            <NodeDeviceSensor
                v-for="(sensorData, sensorName) in deviceMetadata.sensors"
                :key="sensorName"
                :deviceType="deviceMetadata.device_type"
                :sensorName="sensorName"
                :sensorValue="sensorData.current_value"
            ></NodeDeviceSensor>
        </transition-group>
    </div>
</template>

<script>
import NodeDeviceSensor from './NodeDeviceSensor'

export default {
    name: 'NodeDevice',
    props: {
        deviceName: String,
        deviceMetadata: Object,
    },
    components: {
        NodeDeviceSensor
    }
}
</script>

<style>
.node-device-container {
    display: flex;
    flex-direction: column;
    margin: 12px;
    padding: 12px;
    background-color: azure;
    -webkit-box-shadow: 3px 3px 15px 5px #777777; 
    box-shadow: 3px 3px 15px 5px #777777;
    border-radius: 12px;
    text-align: start;
}

.node-device-name {
    margin: 4px;
    margin-bottom: 0px;
    padding-bottom: 8px;
    border-bottom: 1px solid #555555;
    margin-bottom: 8px;
}

.node-device-type {
    font-weight: normal;
    font-size: 0.8em;
}

.node-divisor {
    height: 0px;
    width: 100%;
    background-color: #444444;
}

.device-sensors-container {
    display: flex;
    flex-grow: 1;
    flex-wrap: wrap;
    margin: 0px 0px 0px 0px;
}

.device-list-item {
  display: inline-block;
  margin-right: 10px;
}
.device-list-enter-active,
.device-list-leave-active {
  transition: all 1s ease;
}
.device-list-enter-from,
.device-list-leave-to {
  opacity: 0;
  transform: translateY(30px);
}

</style>