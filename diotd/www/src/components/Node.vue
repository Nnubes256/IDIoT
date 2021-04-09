<template>
    <div class="node-container">
        <div class="node-metadata">
            <h1 class="node-name">{{ name }}</h1>
            <span class="node-id">{{ id }}</span>
        </div>
        <div class="node-devices">
            <NodeDevice
                v-for="(deviceData, deviceName) in devices"
                :key="deviceName"
                :deviceName="deviceName"
                :deviceMetadata="deviceData"></NodeDevice>
        </div>
    </div>
</template>

<script>
import NodeDevice from './NodeDevice.vue'

export default {
  name: 'Node',
  props: {
      name: String,
      id: String,
      devices: Object,
  },
  components: {
      NodeDevice
  }
}
</script>

<style>
.node-container {
    display: flex;
    flex-direction: column;
    margin: 12px;
    padding: 12px;
    background-color: azure;
    -webkit-box-shadow: 3px 3px 15px 5px #777777; 
    box-shadow: 3px 3px 15px 5px #777777;
    border-radius: 12px;
}

.node-metadata {
    display: flex;
    flex-direction: row;
    margin: 0px 16px 0px 16px;
    align-items: baseline;
}

.node-name {
    text-align: start;
    margin: 4px 12px 4px 0px;
}

.node-id {
    text-align: start;
    margin: 4px 0px 4px 0px;
    font-weight: bold;
    font-family: 'Courier New', Courier, monospace;
}

.node-devices {
    display: flex;
    flex-direction: column;
    flex-grow: 1;
}

</style>