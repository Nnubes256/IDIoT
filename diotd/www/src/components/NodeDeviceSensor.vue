<template>
    <div class="sensor-container">
        <h3 class="sensor-name">{{ sensorName }}</h3>
        <div
            v-bind:class="{ 'signal-blob': true, 'signal-blob-animate': animateSignal }"
            v-on:animationend="onSignalAnimationEnd()"
            v-if="this.sensorValue === 'signal' || (this.sensorValue.signal)">!</div>
        <p class="sensor-value">{{ processedMeasurement }}</p>
        <p class="sensor-unit">{{ measurementUnit }}</p>
    </div>
</template>

<script>
const measurementUnits = {
    'dht11': {
        'temperature': '°C',
        'humidity': '%'
    }
}

export default {
    name: 'NodeDeviceSensor',
    data: function() {
        return {
            animateSignal: false
        }
    },
    props: {
        deviceType: String,
        sensorName: String,
        sensorValue: [Object, String],
    },
    methods: {
        updateSignal() {
            console.log("activate!");
            this.animateSignal = true;
        },
        onSignalAnimationEnd() {
            this.animateSignal = false;
        }
    },
    computed: {
        processedMeasurement() {
            if (this.sensorValue === "signal") {
                this.updateSignal();
                return ""
            }

            const measurementTypes = Object.keys(this.sensorValue);

            if (measurementTypes.length == 0) {
                return "N/A";
            }

            if (measurementTypes.length > 1) {
                return "???";
            }

            const measurementType = measurementTypes[0];

             if (measurementType === "signal") {
                this.updateSignal();
                return ""
            }

            return this.sensorValue[measurementType];
        },
        measurementUnit() {
            if (!this.deviceType || !this.sensorName || !(this.deviceType in measurementUnits)) {
                return "";
            }

            const devUnits = measurementUnits[this.deviceType];

            if (!(this.sensorName in devUnits)) {
                return "";
            }

            return devUnits[this.sensorName];
        },
    },
}
</script>

<style>
.sensor-container {
    display: flex;
    flex-direction: column;
    margin: 12px 22px 12px 22px;
    padding: 12px;
    background-color: azure;
    -webkit-box-shadow: 3px 3px 15px 5px #777777; 
    box-shadow: 3px 3px 15px 5px #777777;
    border-radius: 12px;
    text-align: center;
    align-items: center;
    min-width: 16em;
}

.sensor-value {
    font-size: 6.5em;
    margin: 0;
}

.sensor-unit {
    font-size: 2em;
    margin: 4px 0px 4px 0px;
}

.signal-blob {
    /* https://www.florin-pop.com/blog/2019/03/css-pulse-effect/ */
	background: rgba(51, 217, 98, 1);
	border-radius: 50%;
	margin: 25px;
    vertical-align: middle;
    line-height: 160px;
	height: 160px;
	width: 160px;
    font-size: 5em;
    color: rgba(1, 1, 1, 0);
    box-shadow: 0 0 0 10px rgba(51, 217, 98, 0.5);
	transform: scale(1);
}

.signal-blob-animate {
	animation: pulse 1.5s;
}

@keyframes pulse {
	0% {
        color: rgba(1, 1, 1, 0);
		transform: scale(1);
		box-shadow: 0 0 0 10px rgba(51, 217, 98, 0.5);
	}

    10% {
        color: rgba(1, 1, 1, 1);
    }

	70% {
        color: rgba(1, 1, 1, 0);
		transform: scale(0.85);
		box-shadow: 0 0 0 50px rgba(51, 217, 98, 0);
	}

    71% {
        box-shadow: 0 0 0 0 rgba(51, 217, 98, 0);
    }

	100% {
		transform: scale(1);
		box-shadow: 0 0 0 10px rgba(51, 217, 98, 0.5);
	}
}

</style>