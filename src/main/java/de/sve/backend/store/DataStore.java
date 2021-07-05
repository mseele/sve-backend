package de.sve.backend.store;

import java.util.List;

import de.sve.backend.model.events.Event;
import de.sve.backend.model.news.Subscription;

public class DataStore {

	public static Event event(String id) throws Exception {
		try (EventsStore store = new EventsStore()) {
			return store.loadEvent(id);
		}
	}

	public static List<Event> events() throws Exception {
		try (EventsStore store = new EventsStore()) {
			return store.loadEvents();
		}
	}

	public static void save(Event data) throws Exception {
		try (EventsStore store = new EventsStore()) {
			Event event = store.loadEvent(data.id());
			if (event != null) {
				event = event.update(data);
			} else {
				event = data;
			}
			store.saveEvent(event);
		}
	}

	public static void delete(Event event) throws Exception {
		try (EventsStore store = new EventsStore()) {
			store.deleteEvent(event);
		}
	}

	public static Event book(String id, boolean isBooking) throws Exception {
		try (EventsStore store = new EventsStore()) {
			return store.increment(id, isBooking);
		}
	}

	public static void subscribe(Subscription data) throws Exception {
		try (NewsStore store = new NewsStore()) {
			Subscription subscription = store.loadSubscription(data);
			if (subscription != null) {
				subscription = subscription.add(data);
			} else {
				subscription = data;
			}
			store.saveSubscription(subscription);
		}
	}

	public static void unsubscribe(Subscription data) throws Exception {
		try (NewsStore store = new NewsStore()) {
			Subscription subscription = store.loadSubscription(data);
			if (subscription != null) {
				subscription = subscription.remove(data);
				if (subscription.types().size() > 0) {
					store.saveSubscription(subscription);
				} else {
					store.deleteSubsription(subscription);
				}
			}
		}
	}

	public static List<Subscription> subscriptions() throws Exception {
		try (NewsStore store = new NewsStore()) {
			return store.loadSubscriptions();
		}
	}

}
