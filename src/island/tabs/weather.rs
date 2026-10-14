use crate::island::*;

impl App {
    pub(in crate::island) fn save_city(&mut self) {
        if let Some(first) = self.ui.city.matches.first().cloned() {
            return self.choose_city(&first);
        }
        self.settings.weather_city = self.ui.city.input.text().trim().chars().take(60).collect();
        self.settings.weather_lat = None;
        self.settings.weather_lon = None;
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        weather::set_city(&self.settings.weather_city);
        self.ui.city.matches.clear();
        self.end_edit(false);
    }

    pub(in crate::island) fn choose_city(&mut self, city: &weather::City) {
        self.settings.weather_city = city.label.clone();
        self.settings.weather_lat = Some(city.lat);
        self.settings.weather_lon = Some(city.lon);
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        weather::set_place(&city.label, city.lat, city.lon);
        self.ui.city.matches.clear();
        self.kill_timer(Timer::Geo);
        self.end_edit(false);
    }

    pub(in crate::island) fn on_geo(&mut self) {
        self.ui.city.matches = weather::matches();
        if self.ui.tab == view::Tab::Weather {
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn click_weather(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let has_weather = self.ui.weather.is_some();
        if let Some(i) =
            view::hit_city_match(pill, self.scale, x as f32, y as f32, self.ui.city.matches.len(), has_weather)
            && let Some(city) = self.ui.city.matches.get(i).cloned()
        {
            return self.choose_city(&city);
        }
        let on_field = view::hit_city(pill, self.scale, x as f32, y as f32, has_weather);
        if !on_field {
            self.ui.city.matches.clear();
            if self.ui.editing {
                self.end_edit(false);
            }
            return;
        }
        if !self.ui.editing {
            self.ui.city.input.set_text(&self.settings.weather_city);
            self.ui.city.matches.clear();
        }
        self.begin_edit();
        self.repaint_expanded();
    }
}
