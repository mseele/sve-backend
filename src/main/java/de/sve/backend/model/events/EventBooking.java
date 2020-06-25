package de.sve.backend.model.events;

import javax.annotation.Nullable;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class EventBooking {

	public static EventBooking create(String eventId, String firstName, String lastName, String street, String city, String email, String phone, Boolean member, Boolean updates, String comment) {
		return new AutoValue_EventBooking(eventId, firstName, lastName, street, city, email, phone, member, updates, comment);
	}

	public abstract String eventId();

	public abstract String firstName();

	public abstract String lastName();

	public abstract String street();

	public abstract String city();

	public abstract String email();

	@Nullable
	public abstract String phone();

	@Nullable
	public abstract Boolean member();

	@Nullable
	public abstract Boolean updates();

	@Nullable
	public abstract String comments();

	public boolean isMember() {
		return member() != null ? member().booleanValue() : false;
	}

	public Double cost(Event event) {
		return isMember() ? event.costMember() : event.costNonMember();
	}

	public boolean subscribeUpdates() {
		return updates() != null ? updates().booleanValue() : false;
	}

}
