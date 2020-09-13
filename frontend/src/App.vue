<template>
  <v-app id="inspire">
    <v-app-bar app flat dark>
      <v-spacer />
      <v-autocomplete
        v-model="select"
        :items="searchResults"
        :search-input.sync="search"
        class="black--text mx-4"
        label="Search items"
        append-icon="mdi-search"
        cache-items
        hide-no-data
        hide-details
      ></v-autocomplete>
    </v-app-bar>
    <v-navigation-drawer dark permanent app>
      <v-list dense>
        <v-list-item>
          <v-list-item-content>
            <v-list-item-title class="title">Waw</v-list-item-title>
            <v-list-item-subtitle>WOW Auction Watcher</v-list-item-subtitle>
          </v-list-item-content>
        </v-list-item>
        <v-list-group prepend-icon="mdi-incognito" value="true">
          <template v-slot:activator>
            <v-list-item-title>Watchlist</v-list-item-title>
          </template>

          <v-list-item
            v-for="(symbol, i) in chartData"
            :key="String(i)"
            link
            @click="$emit('show-chart-dialog', symbol)"
          >
            <v-list-item-title v-text="symbol.name"></v-list-item-title>
          </v-list-item>
        </v-list-group>
        <v-list-item link>
          <v-list-item-action>
            <v-icon>mdi-email</v-icon>
          </v-list-item-action>
          <v-list-item-content>
            <v-list-item-title>Contact</v-list-item-title>
          </v-list-item-content>
        </v-list-item>
      </v-list>
    </v-navigation-drawer>
    <v-main>
      <v-container align="top" class="fill-height">
        <v-row align="start" justify="center">
          <add-watchlist-form />
        </v-row>
        <v-row align="start" justify="center">
          <v-col align="start" class="text-center">
            <watchlist-table v-if="chartData !== undefined" v-bind:items="chartData" />
          </v-col>
        </v-row>
      </v-container>
    </v-main>

    <v-footer light>
      <span class="black--text">&copy; {{ new Date().getFullYear() }}</span>
    </v-footer>
  </v-app>
</template>

<script lang="ts">
/* eslint-disable vue/no-unused-components */
import Vue from "vue";
import ItemCard from "./components/ItemCard.vue";
import WatchlistTable from "./components/WatchlistTable.vue";
import AddWatchlistForm from "./components/AddWatchlistForm.vue";
import * as d3 from "d3";
import Vuelidate from 'vuelidate'

Vue.use(Vuelidate)

const symbols = Vue.observable([]);
fetch("http://winston:8080/watchlist")
  .then(data => data.json())
  .then((data: number[]) => {
    data.forEach(id => {
      d3.json("http://winston:8080/series/" + id).then(function(data: {
        prices: { value: number }[];
      }) {
        symbols.push({ values: data.prices.map(p => p.value), ...data });
      });
    });
  });

export default Vue.extend({
  name: "App",

  components: {
    "add-watchlist-form": AddWatchlistForm,
    "item-card": ItemCard,
    "watchlist-table": WatchlistTable
  },

  watch: {
    search(newVal, oldVal) {
      return newVal + oldVal;
    }
  },

  data: () => {
    return {
      chartData: symbols,
      searchResults: [],
      select: {},
      search: null,
    };
  },
});
</script>
